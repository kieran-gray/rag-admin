use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::server::application::evaluation::ports::LlmJudge;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationRepository;
use crate::server::domain::evaluation::value_objects::OptimizationBudget;
use crate::shared::{
    evaluation_score, ChunkingConfig, EvaluationMetrics, EvaluationResultSplit,
    EvaluationRunOptions, OptimizationScope,
};
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::event_store::EventStore;

use super::scoring::{PreparedVariant, QuestionSubset, RunContext, TrialScorer};
use crate::server::application::ports::Clock;
use crate::server::application::{
    ActivityRegistry, AppError, InternalLogEvent, Job, JobIdStrategy, JobRegistry, JobSession,
};
use crate::server::domain::evaluation::optimizer::search_space::{
    Fitness, Observation, Parameter, SearchSpace, Trial, Value,
};
use crate::server::domain::evaluation::optimizer::tpe::WARMUP_TRIALS;
use crate::server::domain::evaluation::optimizer::{encoding, halving, Rung, SearchBudget, Tpe};
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{
    AdvanceRung, CompleteRun, EvaluationRunCommand, FailRun, MarkVariantPrepared, ProposeTrial,
    ScoreVariant, SelectChampion,
};
use crate::server::domain::evaluation::run::events::EvaluationRunEvent;
use crate::server::domain::evaluation::split::{
    seed_from_uuid, shuffled_tuning_order, three_way, ThreeWayRatios, ThreeWaySplit,
};

use crate::server::domain::evaluation::run::effects::OptimizeRunEffect;

pub struct OptimizeRunEffectExecutor {
    trial_scorer: Arc<TrialScorer>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    event_store: Arc<dyn EventStore<EvaluationRunEvent>>,
    chunking_configurations: Arc<dyn ChunkingConfigurationRepository>,
    session: JobSession<EvaluationRun>,
    clock: Arc<dyn Clock>,
    judge: Option<Arc<dyn LlmJudge>>,
}

const JUDGE_QUESTION_SAMPLE_SIZE: usize = 5;

const JUDGE_PASSAGE_CHAR_CAP: usize = 8000;

struct TrialOutcome {
    metrics: EvaluationMetrics,
    options: EvaluationRunOptions,
    config: ChunkingConfig,
    chunk_set_id: Uuid,
    embedding_set_id: Uuid,
}

struct PreparedVariantCache {
    entries: Vec<(ChunkingConfig, Arc<PreparedVariant>)>,
    emitted: Vec<ChunkingConfig>,
}

impl PreparedVariantCache {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            emitted: Vec::new(),
        }
    }

    fn find(&self, config: ChunkingConfig) -> Option<Arc<PreparedVariant>> {
        self.entries
            .iter()
            .find(|(c, _)| *c == config)
            .map(|(_, p)| Arc::clone(p))
    }

    fn insert(&mut self, config: ChunkingConfig, prepared: Arc<PreparedVariant>) {
        self.entries.push((config, prepared));
    }

    async fn get_or_prepare(
        &mut self,
        scorer: &TrialScorer,
        ctx: &RunContext,
        config: ChunkingConfig,
        label: String,
    ) -> Result<Arc<PreparedVariant>, AppError> {
        if let Some(existing) = self.find(config) {
            return Ok(existing);
        }
        let prepared = Arc::new(scorer.prepare_variant(ctx, label, config).await?);
        self.insert(config, Arc::clone(&prepared));
        Ok(prepared)
    }

    fn mark_emitted(&mut self, config: ChunkingConfig) -> bool {
        if self.emitted.contains(&config) {
            false
        } else {
            self.emitted.push(config);
            true
        }
    }
}

struct ResumeState {
    trials: HashMap<u32, (HashMap<String, Value>, u32)>,
    scored_metrics: HashMap<(u32, u32), EvaluationMetrics>,
    validation_metrics: HashMap<u32, EvaluationMetrics>,
    holdout_metrics: HashMap<u32, EvaluationMetrics>,
    final_rung_obs: HashMap<u32, Observation>,
    rung_survivors: HashMap<u32, Vec<u32>>,
}

impl OptimizeRunEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trial_scorer: Arc<TrialScorer>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        event_store: Arc<dyn EventStore<EvaluationRunEvent>>,
        chunking_configurations: Arc<dyn ChunkingConfigurationRepository>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
        judge: Option<Arc<dyn LlmJudge>>,
    ) -> Arc<Self> {
        let session = JobSession::new(
            job_registry,
            activity_registry,
            Arc::clone(&command_processor),
            Arc::clone(&clock),
        );
        Arc::new(Self {
            trial_scorer,
            command_processor,
            event_store,
            chunking_configurations,
            session,
            clock,
            judge,
        })
    }

    pub(crate) async fn run(&self, effect: &OptimizeRunEffect) -> Result<(), AppError> {
        let run_id = effect.run_id;
        let clock_for_fail = Arc::clone(&self.clock);
        self.session
            .run(
                run_id,
                JobIdStrategy::Fresh,
                "Optimization failed",
                move |reason| {
                    EvaluationRunCommand::FailRun(FailRun {
                        run_id,
                        reason,
                        occurred_at: clock_for_fail.now(),
                    })
                },
                |job| async move { self.run_inner(effect, &job).await },
            )
            .await
    }

    async fn run_inner(&self, effect: &OptimizeRunEffect, job: &Arc<Job>) -> Result<(), AppError> {
        let budget = budget_from_dto(effect.optimization.budget);
        let scope_shared: OptimizationScope = effect.optimization.scope.into();
        let seed = effect
            .optimization
            .seed
            .unwrap_or_else(|| seed_from_uuid(effect.run_id));
        let mut tpe = Tpe::new(build_default_search_space(scope_shared), seed);

        let resume = self.load_resume_state(effect.run_id).await?;
        self.apply_resume_to_tpe(&resume, &mut tpe, job).await;

        let ctx = self
            .trial_scorer
            .load_run_context(effect.dataset_id, effect.index_profile_id)
            .await?;
        let pinned = self.resolve_pinned_chunking(effect).await?;
        let split = three_way(seed, ctx.questions.len(), ThreeWayRatios::default());
        if !split.is_usable() {
            return Err(AppError::Validation(format!(
                "optimization needs at least 3 questions for a three-way split (got {})",
                ctx.questions.len(),
            )));
        }
        log_optimization_start(effect, job, &split).await;

        let mut prepared_cache = PreparedVariantCache::new();
        self.prepare_pinned_variant(effect, job, &ctx, pinned, &mut prepared_cache)
            .await?;

        let (outcomes, active_trial_ids) = self
            .run_tuning_rungs(
                effect,
                job,
                &ctx,
                &mut tpe,
                budget,
                pinned,
                &mut prepared_cache,
                &resume,
                &split.tuning,
            )
            .await?;

        let final_scored = top_survivors(&active_trial_ids, &outcomes, budget.holdout_top_n());

        let validation_subset = QuestionSubset::from_indices(
            &ctx.questions,
            &ctx.question_embeddings,
            &split.validation,
        );
        let holdout_subset =
            QuestionSubset::from_indices(&ctx.questions, &ctx.question_embeddings, &split.holdout);

        let judge_scores = self
            .run_judge_pass(
                effect,
                job,
                &ctx,
                &prepared_cache,
                &final_scored,
                &outcomes,
                &split.validation,
            )
            .await;

        let validation_scores = self
            .run_validation_pass(
                effect,
                job,
                &final_scored,
                &outcomes,
                &prepared_cache,
                &validation_subset,
                &resume,
                &judge_scores,
                split.validation.len(),
            )
            .await?;

        let champion = self
            .select_and_score_champion(
                effect,
                job,
                &validation_scores,
                &outcomes,
                &prepared_cache,
                &holdout_subset,
                &resume,
                split.holdout.len(),
            )
            .await?;

        self.complete_optimization(
            effect,
            job,
            champion,
            outcomes.len(),
            budget.schedule().len(),
        )
        .await
    }

    async fn apply_resume_to_tpe(&self, resume: &ResumeState, tpe: &mut Tpe, job: &Arc<Job>) {
        if resume.trials.is_empty() {
            return;
        }
        job.emit(
            InternalLogEvent::info(format!(
                "Resuming run: {} trials proposed, {} tuning scores + {} holdout scores already persisted",
                resume.trials.len(),
                resume.scored_metrics.len(),
                resume.holdout_metrics.len(),
            ))
            .with_meta("resumed_trials", json!(resume.trials.len())),
        )
        .await;
        tpe.skip_trial_ids(
            resume
                .trials
                .keys()
                .copied()
                .max()
                .map(|m| m + 1)
                .unwrap_or(0),
        );
        let seeded: Vec<Observation> = resume.final_rung_obs.values().cloned().collect();
        tpe.observe(&seeded);
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_tuning_rungs(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        ctx: &RunContext,
        tpe: &mut Tpe,
        budget: SearchBudget,
        pinned: Option<ChunkingConfig>,
        cache: &mut PreparedVariantCache,
        resume: &ResumeState,
        tuning_indices: &[usize],
    ) -> Result<(HashMap<u32, TrialOutcome>, Vec<u32>), AppError> {
        let schedule = budget.schedule();
        let mut outcomes: HashMap<u32, TrialOutcome> = HashMap::new();
        let mut active_trial_ids: Vec<u32> = Vec::new();
        let tuning_order = shuffled_tuning_order(tuning_indices, effect.run_id);
        let mut proposed_params: HashMap<u32, HashMap<String, Value>> = resume
            .trials
            .iter()
            .map(|(tid, (p, _))| (*tid, p.clone()))
            .collect();

        for (rung_idx, rung) in schedule.iter().enumerate() {
            let rung_num = (rung_idx + 1) as u32;
            let subset = build_rung_subset(ctx, &tuning_order, *rung);

            let batches = plan_rung_batches(
                rung_idx,
                rung.trials,
                resume.trials.is_empty(),
                &resume.trials,
                &active_trial_ids,
                &proposed_params,
            );

            let total_trials: usize = batches.iter().map(RungBatch::size).sum();
            job.emit(
                InternalLogEvent::info(format!(
                    "Rung {rung_num}: scoring {total_trials} trials across {} batch(es) on {} questions",
                    batches.len(),
                    subset.questions.len(),
                ))
                .with_meta("rung", json!(rung_num))
                .with_meta("trials", json!(total_trials))
                .with_meta("batches", json!(batches.len()))
                .with_meta("questions", json!(subset.questions.len())),
            )
            .await;

            let mut rung_obs: Vec<Observation> = Vec::with_capacity(total_trials);
            for batch in batches {
                let proposals: Vec<Trial> = match batch {
                    RungBatch::FreshPropose(n) => {
                        let new_trials = tpe.propose(n);
                        for trial in &new_trials {
                            proposed_params.insert(trial.trial_id, trial.params.clone());
                            self.command_processor
                                .handle(
                                    effect.run_id,
                                    EvaluationRunCommand::ProposeTrial(ProposeTrial {
                                        run_id: effect.run_id,
                                        trial_id: trial.trial_id,
                                        params: encoding::params_to_json(&trial.params),
                                        rung: rung_num,
                                        occurred_at: self.clock.now(),
                                    }),
                                )
                                .await?;
                        }
                        new_trials
                    }
                    RungBatch::Replay(trials) => trials,
                };

                let mut batch_obs: Vec<Observation> = Vec::with_capacity(proposals.len());
                for trial in &proposals {
                    let (config, options) = match pinned {
                        Some(pinned_config) => (
                            pinned_config,
                            encoding::retrieval_params_to_options(&trial.params),
                        ),
                        None => encoding::params_to_run_config(
                            &trial.params,
                            ctx.generation_model.generation_model_id,
                        ),
                    };
                    let prepared = cache
                        .get_or_prepare(
                            &self.trial_scorer,
                            ctx,
                            config,
                            encoding::trial_label(trial.trial_id),
                        )
                        .await?;

                    if cache.mark_emitted(config) {
                        self.command_processor
                            .handle(
                                effect.run_id,
                                EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                                    run_id: effect.run_id,
                                    variant_label: prepared.label.clone(),
                                    chunk_set_id: prepared.chunk_set_id,
                                    embedding_set_id: prepared.embedding_set_id,
                                    occurred_at: self.clock.now(),
                                }),
                            )
                            .await?;
                    }

                    let (metrics, composite) =
                        if let Some(m) = resume.scored_metrics.get(&(trial.trial_id, rung_num)) {
                            let m = m.clone();
                            let c = evaluation_score(&m);
                            (m, c)
                        } else {
                            let (m, t) = self
                                .trial_scorer
                                .score_variant(effect.run_id, &prepared, &subset, &options)
                                .await?;
                            let c = evaluation_score(&m);
                            self.command_processor
                                .handle(
                                    effect.run_id,
                                    EvaluationRunCommand::ScoreVariant(ScoreVariant {
                                        run_id: effect.run_id,
                                        variant_label: encoding::trial_rung_label(
                                            trial.trial_id,
                                            rung_num,
                                        ),
                                        variant_config: prepared.config.into(),
                                        options: options.clone().into(),
                                        split: EvaluationResultSplit::Tuning.into(),
                                        chunk_set_id: prepared.chunk_set_id,
                                        embedding_set_id: prepared.embedding_set_id,
                                        metrics: m.clone().into(),
                                        retrieval_traces: t,
                                        selected: false,
                                        occurred_at: self.clock.now(),
                                    }),
                                )
                                .await?;
                            (m, c)
                        };

                    let fitness = Fitness {
                        composite,
                        composite_ci: (metrics.composite_ci_low, metrics.composite_ci_high),
                        recall: metrics.recall_mean,
                        precision: metrics.precision_mean,
                        iou: metrics.iou_mean,
                        precision_omega: metrics.precision_omega_mean,
                        cost: metrics.average_retrieved_tokens as f32,
                        judge_score: None,
                    };
                    batch_obs.push(Observation {
                        trial_id: trial.trial_id,
                        params: trial.params.clone(),
                        fitness,
                    });

                    outcomes.insert(
                        trial.trial_id,
                        TrialOutcome {
                            metrics,
                            options: options.clone(),
                            config: prepared.config,
                            chunk_set_id: prepared.chunk_set_id,
                            embedding_set_id: prepared.embedding_set_id,
                        },
                    );
                }
                tpe.observe(&batch_obs);
                rung_obs.extend(batch_obs);
            }

            let scored: Vec<(u32, f32)> = rung_obs
                .iter()
                .map(|o| (o.trial_id, o.fitness.composite))
                .collect();
            let take = schedule
                .get(rung_idx + 1)
                .map(|r| r.trials)
                .unwrap_or(budget.holdout_top_n());

            let survivors = match resume.rung_survivors.get(&rung_num) {
                Some(historical) => historical.clone(),
                None => halving::survivors(&scored, take),
            };

            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::AdvanceRung(AdvanceRung {
                        run_id: effect.run_id,
                        rung: rung_num,
                        surviving_trials: survivors.clone(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;

            active_trial_ids = survivors;

            job.emit(
                InternalLogEvent::info(format!(
                    "Rung {rung_num} complete: {} survivors → next stage",
                    active_trial_ids.len(),
                ))
                .with_meta("rung", json!(rung_num))
                .with_meta("survivors", json!(active_trial_ids.len())),
            )
            .await;
        }

        Ok((outcomes, active_trial_ids))
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_validation_pass(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        final_scored: &[(u32, f32)],
        outcomes: &HashMap<u32, TrialOutcome>,
        cache: &PreparedVariantCache,
        validation_subset: &QuestionSubset<'_>,
        resume: &ResumeState,
        judge_scores: &HashMap<u32, f32>,
        validation_count: usize,
    ) -> Result<HashMap<u32, EvaluationMetrics>, AppError> {
        job.emit(
            InternalLogEvent::info(format!(
                "Validation pass: scoring top {} survivors on {validation_count} validation questions",
                final_scored.len(),
            ))
            .with_meta("top_n", json!(final_scored.len())),
        )
        .await;

        let mut validation_scores: HashMap<u32, EvaluationMetrics> =
            resume.validation_metrics.clone();
        for (trial_id, _) in final_scored {
            let Some(outcome) = outcomes.get(trial_id) else {
                continue;
            };
            let config = outcome.config;
            let options = outcome.options.clone();
            let chunk_set_id = outcome.chunk_set_id;
            let embedding_set_id = outcome.embedding_set_id;

            let (mut metrics, traces) = if let Some(m) = resume.validation_metrics.get(trial_id) {
                (m.clone(), Vec::new())
            } else {
                let Some(prepared) = cache.find(config) else {
                    continue;
                };
                self.trial_scorer
                    .score_variant(effect.run_id, &prepared, validation_subset, &options)
                    .await?
            };
            metrics.judge_score = judge_scores.get(trial_id).copied();

            validation_scores.insert(*trial_id, metrics.clone());

            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::ScoreVariant(ScoreVariant {
                        run_id: effect.run_id,
                        variant_label: encoding::trial_validation_label(*trial_id),
                        variant_config: config.into(),
                        options: options.into(),
                        split: EvaluationResultSplit::Validation.into(),
                        chunk_set_id,
                        embedding_set_id,
                        metrics: metrics.into(),
                        retrieval_traces: traces,
                        selected: false,
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
        }
        Ok(validation_scores)
    }

    #[allow(clippy::too_many_arguments)]
    async fn select_and_score_champion(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        validation_scores: &HashMap<u32, EvaluationMetrics>,
        outcomes: &HashMap<u32, TrialOutcome>,
        cache: &PreparedVariantCache,
        holdout_subset: &QuestionSubset<'_>,
        resume: &ResumeState,
        holdout_count: usize,
    ) -> Result<Option<(u32, EvaluationMetrics)>, AppError> {
        let Some(trial_id) = validation_scores
            .iter()
            .max_by(|a, b| {
                evaluation_score(a.1)
                    .partial_cmp(&evaluation_score(b.1))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|(tid, _)| *tid)
        else {
            return Ok(None);
        };
        let Some(outcome) = outcomes.get(&trial_id) else {
            return Ok(None);
        };
        let Some(prepared) = cache.find(outcome.config) else {
            return Ok(None);
        };
        let config = outcome.config;
        let options = outcome.options.clone();
        let chunk_set_id = outcome.chunk_set_id;
        let embedding_set_id = outcome.embedding_set_id;

        job.emit(
            InternalLogEvent::info(format!(
                "Holdout integrity pass: scoring champion (trial {trial_id}) on {holdout_count} holdout questions",
            ))
            .with_meta("trial_id", json!(trial_id))
            .with_meta("holdout_question_count", json!(holdout_count)),
        )
        .await;

        let prior_holdout = resume.holdout_metrics.get(&trial_id).cloned();
        let already_scored = prior_holdout.is_some();
        let (metrics, traces) = if let Some(m) = prior_holdout {
            (m, Vec::new())
        } else {
            self.trial_scorer
                .score_variant(effect.run_id, &prepared, holdout_subset, &options)
                .await?
        };

        if !already_scored {
            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::ScoreVariant(ScoreVariant {
                        run_id: effect.run_id,
                        variant_label: encoding::trial_holdout_label(trial_id),
                        variant_config: config.into(),
                        options: options.into(),
                        split: EvaluationResultSplit::Holdout.into(),
                        chunk_set_id,
                        embedding_set_id,
                        metrics: metrics.clone().into(),
                        retrieval_traces: traces,
                        selected: true,
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
        }
        Ok(Some((trial_id, metrics)))
    }

    async fn complete_optimization(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        champion: Option<(u32, EvaluationMetrics)>,
        trial_count: usize,
        rung_count: usize,
    ) -> Result<(), AppError> {
        if let Some((trial_id, metrics)) = champion {
            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::SelectChampion(SelectChampion {
                        run_id: effect.run_id,
                        trial_id,
                        holdout_metrics: metrics.into(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
        }

        self.command_processor
            .handle(
                effect.run_id,
                EvaluationRunCommand::CompleteRun(CompleteRun {
                    run_id: effect.run_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        job.emit(
            InternalLogEvent::success(format!(
                "Optimization complete · {trial_count} trials across {rung_count} rungs",
            ))
            .with_meta("run_id", json!(effect.run_id.to_string()))
            .with_meta("trial_count", json!(trial_count)),
        )
        .await;
        Ok(())
    }

    async fn load_resume_state(&self, run_id: Uuid) -> Result<ResumeState, AppError> {
        let envelopes = self.event_store.load(run_id).await?;
        let mut trials: HashMap<u32, (HashMap<String, Value>, u32)> = HashMap::new();
        let mut scored_metrics: HashMap<(u32, u32), EvaluationMetrics> = HashMap::new();
        let mut validation_metrics: HashMap<u32, EvaluationMetrics> = HashMap::new();
        let mut holdout_metrics: HashMap<u32, EvaluationMetrics> = HashMap::new();
        let mut final_rung_obs: HashMap<u32, Observation> = HashMap::new();
        let mut highest_seen_rung: HashMap<u32, u32> = HashMap::new();
        let mut rung_survivors: HashMap<u32, Vec<u32>> = HashMap::new();

        for env in &envelopes {
            match &env.event {
                EvaluationRunEvent::TrialProposed(t) => {
                    let params = encoding::params_from_json(&t.params);
                    trials.insert(t.trial_id, (params, t.rung));
                }
                EvaluationRunEvent::VariantScored(s) => {
                    let metrics: EvaluationMetrics = s.metrics.clone().into();
                    if let Some((trial_id, rung)) =
                        encoding::parse_trial_rung_label(&s.variant_label)
                    {
                        scored_metrics.insert((trial_id, rung), metrics.clone());
                        let params = trials
                            .get(&trial_id)
                            .map(|(p, _)| p.clone())
                            .unwrap_or_default();
                        let fitness = Fitness {
                            composite: evaluation_score(&metrics),
                            composite_ci: (metrics.composite_ci_low, metrics.composite_ci_high),
                            recall: metrics.recall_mean,
                            precision: metrics.precision_mean,
                            iou: metrics.iou_mean,
                            precision_omega: metrics.precision_omega_mean,
                            cost: metrics.average_retrieved_tokens as f32,
                            judge_score: None,
                        };

                        let upgrade = highest_seen_rung
                            .get(&trial_id)
                            .map(|prev| rung >= *prev)
                            .unwrap_or(true);
                        if upgrade {
                            highest_seen_rung.insert(trial_id, rung);
                            final_rung_obs.insert(
                                trial_id,
                                Observation {
                                    trial_id,
                                    params,
                                    fitness,
                                },
                            );
                        }
                    } else if let Some(trial_id) =
                        encoding::parse_trial_validation_label(&s.variant_label)
                    {
                        validation_metrics.insert(trial_id, metrics);
                    } else if let Some(trial_id) =
                        encoding::parse_trial_holdout_label(&s.variant_label)
                    {
                        holdout_metrics.insert(trial_id, metrics);
                    }
                }
                EvaluationRunEvent::RungAdvanced(r) => {
                    rung_survivors.insert(r.rung, r.surviving_trials.clone());
                }
                _ => {}
            }
        }

        Ok(ResumeState {
            trials,
            scored_metrics,
            validation_metrics,
            holdout_metrics,
            final_rung_obs,
            rung_survivors,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_judge_pass(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        ctx: &RunContext,
        prepared_cache: &PreparedVariantCache,
        final_scored: &[(u32, f32)],
        outcomes: &HashMap<u32, TrialOutcome>,
        holdout: &[usize],
    ) -> HashMap<u32, f32> {
        let mut judge_scores: HashMap<u32, f32> = HashMap::new();
        if !effect.optimization.judges_enabled {
            return judge_scores;
        }
        let Some(judge) = &self.judge else {
            job.emit(InternalLogEvent::warn(
                "LLM judge requested but no adapter is wired — skipping".to_string(),
            ))
            .await;
            return judge_scores;
        };

        let model_id = ctx.generation_model.generation_model_id;
        let sample = pick_judge_sample(holdout, JUDGE_QUESTION_SAMPLE_SIZE);
        job.emit(InternalLogEvent::info(format!(
            "LLM judge enabled · scoring {} survivor(s) on {} holdout questions",
            final_scored.len(),
            sample.len(),
        )))
        .await;

        for (trial_id, _) in final_scored {
            let Some(outcome) = outcomes.get(trial_id) else {
                continue;
            };
            let options = outcome.options.clone();
            let Some(prepared) = prepared_cache.find(outcome.config) else {
                continue;
            };

            let mut accum = 0.0f32;
            let mut counted = 0usize;
            for &q_idx in &sample {
                let (Some(question), Some(q_emb)) =
                    (ctx.questions.get(q_idx), ctx.question_embeddings.get(q_idx))
                else {
                    continue;
                };
                let retrieved = match self
                    .trial_scorer
                    .retrieve_passage(&prepared, q_emb, &options)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        job.emit(InternalLogEvent::warn(format!(
                            "Judge retrieval failed for trial {trial_id}: {e}",
                        )))
                        .await;
                        continue;
                    }
                };
                let passage = chunks_to_passage(&retrieved);
                match judge
                    .judge_context(model_id, &question.question, &passage)
                    .await
                {
                    Ok(verdict) => {
                        let sufficient = if verdict.sufficient { 1.0 } else { 0.0 };
                        accum += sufficient * verdict.confidence;
                        counted += 1;
                    }
                    Err(e) => {
                        job.emit(InternalLogEvent::warn(format!(
                            "Judge call failed for trial {trial_id}: {e}",
                        )))
                        .await;
                    }
                }
            }
            if counted > 0 {
                let score = (accum / counted as f32).clamp(0.0, 1.0);
                judge_scores.insert(*trial_id, score);
                job.emit(
                    InternalLogEvent::info(format!("Trial {trial_id} judge score = {score:.2}"))
                        .with_meta("trial_id", json!(trial_id))
                        .with_meta("judge_score", json!(score)),
                )
                .await;
            }
        }
        judge_scores
    }

    async fn prepare_pinned_variant(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        ctx: &RunContext,
        pinned: Option<ChunkingConfig>,
        cache: &mut PreparedVariantCache,
    ) -> Result<(), AppError> {
        let Some(pinned_config) = pinned else {
            return Ok(());
        };
        let prepared = Arc::new(
            self.trial_scorer
                .prepare_variant(ctx, encoding::pinned_label(), pinned_config)
                .await?,
        );
        self.command_processor
            .handle(
                effect.run_id,
                EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                    run_id: effect.run_id,
                    variant_label: prepared.label.clone(),
                    chunk_set_id: prepared.chunk_set_id,
                    embedding_set_id: prepared.embedding_set_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        job.emit(
            InternalLogEvent::info(format!(
                "Pinned chunking '{}' prepared: {} chunks · reusing for every trial",
                prepared.label,
                prepared.chunks.len(),
            ))
            .with_meta("variant_label", json!(prepared.label))
            .with_meta("chunk_count", json!(prepared.chunks.len()))
            .with_meta("chunk_set_id", json!(prepared.chunk_set_id.to_string())),
        )
        .await;
        cache.mark_emitted(pinned_config);
        cache.insert(pinned_config, prepared);
        Ok(())
    }

    async fn resolve_pinned_chunking(
        &self,
        effect: &OptimizeRunEffect,
    ) -> Result<Option<ChunkingConfig>, AppError> {
        let Some(id) = effect.optimization.fixed_chunking_configuration_id else {
            return Ok(None);
        };
        let entry = self
            .chunking_configurations
            .find_by_id(id)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!("chunking configuration {id} not found"))
            })?;
        Ok(Some(entry.config.into()))
    }

}

fn budget_from_dto(b: OptimizationBudget) -> SearchBudget {
    match b {
        OptimizationBudget::Quick => SearchBudget::Quick,
        OptimizationBudget::Thorough => SearchBudget::Thorough,
        OptimizationBudget::Exhaustive => SearchBudget::Exhaustive,
    }
}

async fn log_optimization_start(
    effect: &OptimizeRunEffect,
    job: &Arc<Job>,
    split: &ThreeWaySplit,
) {
    job.emit(
        InternalLogEvent::info(format!(
            "Starting optimization: budget={:?} scope={:?} judges={} · tuning {} / validation {} / holdout {}",
            effect.optimization.budget,
            effect.optimization.scope,
            effect.optimization.judges_enabled,
            split.tuning.len(),
            split.validation.len(),
            split.holdout.len(),
        ))
        .with_meta("run_id", json!(effect.run_id.to_string()))
        .with_meta("tuning_question_count", json!(split.tuning.len()))
        .with_meta("validation_question_count", json!(split.validation.len()))
        .with_meta("holdout_question_count", json!(split.holdout.len())),
    )
    .await;
}

fn top_survivors(
    active_trial_ids: &[u32],
    outcomes: &HashMap<u32, TrialOutcome>,
    top_n: usize,
) -> Vec<(u32, f32)> {
    let mut scored: Vec<(u32, f32)> = active_trial_ids
        .iter()
        .filter_map(|tid| outcomes.get(tid).map(|o| (*tid, evaluation_score(&o.metrics))))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    scored.truncate(top_n);
    scored
}

fn pick_judge_sample(pool: &[usize], n: usize) -> Vec<usize> {
    pool.iter().take(n).copied().collect()
}

fn chunks_to_passage(retrieved: &[(Uuid, f32, String)]) -> String {
    let mut joined = String::new();
    for (_, _, text) in retrieved {
        if !joined.is_empty() {
            joined.push_str("\n\n---\n\n");
        }
        joined.push_str(text);
        if joined.len() > JUDGE_PASSAGE_CHAR_CAP {
            joined.truncate(JUDGE_PASSAGE_CHAR_CAP);
            joined.push_str("\n\n[truncated]");
            break;
        }
    }
    joined
}

enum RungBatch {
    FreshPropose(usize),
    Replay(Vec<Trial>),
}

impl RungBatch {
    fn size(&self) -> usize {
        match self {
            RungBatch::FreshPropose(n) => *n,
            RungBatch::Replay(trials) => trials.len(),
        }
    }
}

fn plan_rung_batches(
    rung_idx: usize,
    rung_trials: usize,
    rung_zero_is_fresh: bool,
    resume_trials: &HashMap<u32, (HashMap<String, Value>, u32)>,
    active_trial_ids: &[u32],
    proposed_params: &HashMap<u32, HashMap<String, Value>>,
) -> Vec<RungBatch> {
    if rung_idx == 0 {
        if !rung_zero_is_fresh {
            let mut replay: Vec<Trial> = resume_trials
                .iter()
                .map(|(tid, (params, _))| Trial {
                    trial_id: *tid,
                    params: params.clone(),
                })
                .collect();
            replay.sort_by_key(|t| t.trial_id);
            return vec![RungBatch::Replay(replay)];
        }
        let warmup_size = WARMUP_TRIALS.min(rung_trials);
        let adaptive_size = rung_trials.saturating_sub(warmup_size);
        let mut batches = Vec::new();
        if warmup_size > 0 {
            batches.push(RungBatch::FreshPropose(warmup_size));
        }
        if adaptive_size > 0 {
            batches.push(RungBatch::FreshPropose(adaptive_size));
        }
        return batches;
    }

    let survivors: Vec<Trial> = active_trial_ids
        .iter()
        .filter_map(|tid| {
            proposed_params.get(tid).map(|params| Trial {
                trial_id: *tid,
                params: params.clone(),
            })
        })
        .collect();
    vec![RungBatch::Replay(survivors)]
}

fn build_rung_subset<'a>(
    ctx: &'a RunContext,
    tuning_order: &[usize],
    rung: Rung,
) -> QuestionSubset<'a> {
    let take = rung
        .question_count(tuning_order.len())
        .min(tuning_order.len());
    let picked: &[usize] = tuning_order.get(..take).unwrap_or_default();
    QuestionSubset::from_indices(&ctx.questions, &ctx.question_embeddings, picked)
}

pub fn build_default_search_space(scope: OptimizationScope) -> SearchSpace {
    let mut params: Vec<Parameter> = Vec::new();

    if matches!(scope, OptimizationScope::Chunking | OptimizationScope::Both) {
        params.push(Parameter::Categorical {
            name: "strategy".into(),
            values: vec!["section".into(), "bert".into(), "llm".into(), "darn".into()],
        });
        params.push(Parameter::Conditional {
            gate_parameter: "strategy".into(),
            gate_value: Value::String("section".into()),
            inner: Box::new(Parameter::IntRange {
                name: "max_section_tokens".into(),
                low: 128,
                high: 1024,
                log_scale: true,
            }),
        });
        params.push(Parameter::Conditional {
            gate_parameter: "strategy".into(),
            gate_value: Value::String("bert".into()),
            inner: Box::new(Parameter::IntRange {
                name: "bert_target_tokens".into(),
                low: 128,
                high: 1024,
                log_scale: true,
            }),
        });
        params.push(Parameter::Conditional {
            gate_parameter: "strategy".into(),
            gate_value: Value::String("llm".into()),
            inner: Box::new(Parameter::IntRange {
                name: "micro_chunk_tokens".into(),
                low: 32,
                high: 512,
                log_scale: true,
            }),
        });
        params.push(Parameter::Conditional {
            gate_parameter: "strategy".into(),
            gate_value: Value::String("darn".into()),
            inner: Box::new(Parameter::IntRange {
                name: "darn_max_chunk_size".into(),
                low: 128,
                high: 2048,
                log_scale: true,
            }),
        });
    }

    if matches!(
        scope,
        OptimizationScope::Retrieval | OptimizationScope::Both
    ) {
        params.push(Parameter::IntRange {
            name: "top_k".into(),
            low: 1,
            high: 10,
            log_scale: false,
        });
        params.push(Parameter::Float {
            name: "min_score".into(),
            low: 0.0,
            high: 0.95,
            log_scale: false,
        });
    }

    SearchSpace::new(params)
}
