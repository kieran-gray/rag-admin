use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::core::{
    evaluation_score, ChunkingConfig, EvaluationMetrics, EvaluationResultSplit,
    EvaluationRunOptions, OptimizationBudget, OptimizationScope,
};
use crate::server::application::evaluation::ports::LlmJudge;
use crate::server::application::evaluation::scoring::{
    PreparedVariant, QuestionSubset, RunContext, TrialScorer,
};
use crate::server::application::ports::Clock;
use crate::server::application::{ActivityRegistry, AppError, InternalLogEvent, Job, JobRegistry};
use crate::server::domain::evaluation::optimizer::search_space::{
    Fitness, Observation, Parameter, SearchSpace, Trial, Value,
};
use crate::server::domain::evaluation::optimizer::tpe::WARMUP_TRIALS;
use crate::server::domain::evaluation::optimizer::{encoding, halving, SearchBudget, Tpe};
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{
    AdvanceRung, CompleteRun, EvaluationRunCommand, MarkVariantPrepared, ProposeTrial,
    ScoreVariant, SelectChampion,
};
use crate::server::domain::evaluation::run::events::EvaluationRunEvent;
use crate::server::domain::evaluation::split::{three_way, ThreeWayRatios};
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::event_store::EventStore;

use super::run::OptimizeRunEffect;
use super::run_session::RunSession;

pub struct OptimizeRunEffectExecutor {
    trial_scorer: Arc<TrialScorer>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    event_store: Arc<dyn EventStore<EvaluationRunEvent>>,
    session: RunSession,
    clock: Arc<dyn Clock>,
    judge: Option<Arc<dyn LlmJudge>>,
}

const JUDGE_QUESTION_SAMPLE_SIZE: usize = 5;

const JUDGE_PASSAGE_CHAR_CAP: usize = 8000;

struct ResumeState {
    trials: HashMap<u32, (HashMap<String, Value>, u32)>,
    scored_tuning: HashSet<(u32, u32)>,
    scored_validation: HashSet<u32>,
    scored_holdout: HashSet<u32>,
    holdout_metrics: HashMap<u32, EvaluationMetrics>,
    scored_metrics: HashMap<(u32, u32), EvaluationMetrics>,
    final_rung_obs: HashMap<u32, Observation>,
    validation_metrics: HashMap<u32, EvaluationMetrics>,
    rung_survivors: HashMap<u32, Vec<u32>>,
    #[allow(dead_code)]
    has_champion: bool,
}

impl OptimizeRunEffectExecutor {
    pub fn new(
        trial_scorer: Arc<TrialScorer>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        event_store: Arc<dyn EventStore<EvaluationRunEvent>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
        judge: Option<Arc<dyn LlmJudge>>,
    ) -> Arc<Self> {
        let session = RunSession::new(
            job_registry,
            activity_registry,
            Arc::clone(&command_processor),
            Arc::clone(&clock),
        );
        Arc::new(Self {
            trial_scorer,
            command_processor,
            event_store,
            session,
            clock,
            judge,
        })
    }

    pub(crate) async fn run(&self, effect: &OptimizeRunEffect) -> Result<(), AppError> {
        self.session
            .run(effect.run_id, "Optimization failed", |job| async move {
                self.run_inner(effect, &job).await
            })
            .await
    }

    async fn run_inner(&self, effect: &OptimizeRunEffect, job: &Arc<Job>) -> Result<(), AppError> {
        let budget: SearchBudget = match effect.optimization.budget {
            OptimizationBudget::Quick => SearchBudget::Quick,
            OptimizationBudget::Thorough => SearchBudget::Thorough,
            OptimizationBudget::Exhaustive => SearchBudget::Exhaustive,
        };
        let space = build_default_search_space(effect.optimization.scope);
        let seed = effect
            .optimization
            .seed
            .unwrap_or_else(|| uuid_to_seed(effect.run_id));
        let mut tpe = Tpe::new(space, seed);

        let resume = self.load_resume_state(effect.run_id).await?;
        if !resume.trials.is_empty() {
            job.emit(
                InternalLogEvent::info(format!(
                    "Resuming run: {} trials proposed, {} tuning scores + {} holdout scores already persisted",
                    resume.trials.len(),
                    resume.scored_tuning.len(),
                    resume.scored_holdout.len(),
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

        let ctx = self
            .trial_scorer
            .load_run_context(effect.dataset_id, effect.pipeline_configuration_id)
            .await?;

        let split = three_way(
            effect.run_id,
            ctx.questions.len(),
            ThreeWayRatios::default(),
        );
        if !split.is_usable() {
            return Err(AppError::Validation(format!(
                "optimization needs at least 3 questions for a three-way split (got {})",
                ctx.questions.len(),
            )));
        }

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

        let schedule = budget.schedule();
        let mut prepared_cache: Vec<(ChunkingConfig, Arc<PreparedVariant>)> = Vec::new();
        let mut prepared_emitted: Vec<ChunkingConfig> = Vec::new();
        let mut last_metrics: HashMap<u32, EvaluationMetrics> = HashMap::new();
        let mut last_options: HashMap<u32, EvaluationRunOptions> = HashMap::new();
        let mut last_config: HashMap<u32, ChunkingConfig> = HashMap::new();
        let mut last_chunk_set: HashMap<u32, Uuid> = HashMap::new();
        let mut last_embedding_set: HashMap<u32, Uuid> = HashMap::new();
        let mut active_trial_ids: Vec<u32> = Vec::new();
        let tuning_order = shuffled_tuning_order(&split.tuning, effect.run_id);

        let mut proposed_params: HashMap<u32, HashMap<String, Value>> = resume
            .trials
            .iter()
            .map(|(tid, (p, _))| (*tid, p.clone()))
            .collect();

        for (rung_idx, rung) in schedule.iter().enumerate() {
            let rung_num = (rung_idx + 1) as u32;
            let subset = build_rung_subset(&ctx, &tuning_order, *rung);

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
                    let (config, options) = encoding::params_to_run_config(
                        &trial.params,
                        ctx.generation_model.generation_model_id,
                    );
                    let prepared = self
                        .ensure_prepared(&ctx, &mut prepared_cache, config, trial.trial_id)
                        .await?;

                    if !prepared_emitted.contains(&config) {
                        prepared_emitted.push(config);
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

                    let (metrics, composite, traces) =
                        if resume.scored_tuning.contains(&(trial.trial_id, rung_num)) {
                            let m = resume
                                .scored_metrics
                                .get(&(trial.trial_id, rung_num))
                                .cloned()
                                .unwrap_or_else(empty_metrics);
                            let c = evaluation_score(&m);
                            (m, c, Vec::new())
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
                                        variant_config: prepared.config,
                                        options: options.clone(),
                                        split: EvaluationResultSplit::Tuning,
                                        chunk_set_id: prepared.chunk_set_id,
                                        embedding_set_id: prepared.embedding_set_id,
                                        metrics: m.clone(),
                                        retrieval_traces: t.clone(),
                                        selected: false,
                                        occurred_at: self.clock.now(),
                                    }),
                                )
                                .await?;
                            (m, c, t)
                        };
                    let _ = traces;

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

                    last_metrics.insert(trial.trial_id, metrics);
                    last_options.insert(trial.trial_id, options.clone());
                    last_config.insert(trial.trial_id, prepared.config);
                    last_chunk_set.insert(trial.trial_id, prepared.chunk_set_id);
                    last_embedding_set.insert(trial.trial_id, prepared.embedding_set_id);
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

        let holdout_top_n = budget.holdout_top_n();
        let mut final_scored: Vec<(u32, f32)> = active_trial_ids
            .iter()
            .filter_map(|tid| last_metrics.get(tid).map(|m| (*tid, evaluation_score(m))))
            .collect();
        final_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        final_scored.truncate(holdout_top_n);

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
                &last_config,
                &last_options,
                &split.validation,
            )
            .await;

        job.emit(
            InternalLogEvent::info(format!(
                "Validation pass: scoring top {} survivors on {} validation questions",
                final_scored.len(),
                split.validation.len(),
            ))
            .with_meta("top_n", json!(final_scored.len())),
        )
        .await;

        let mut validation_scores: HashMap<u32, EvaluationMetrics> =
            resume.validation_metrics.clone();
        for (trial_id, _) in &final_scored {
            let (Some(config), Some(options), Some(chunk_set_id), Some(embedding_set_id)) = (
                last_config.get(trial_id).copied(),
                last_options.get(trial_id).cloned(),
                last_chunk_set.get(trial_id).copied(),
                last_embedding_set.get(trial_id).copied(),
            ) else {
                continue;
            };

            let (mut metrics, traces) = if resume.scored_validation.contains(trial_id) {
                let m = resume
                    .validation_metrics
                    .get(trial_id)
                    .cloned()
                    .unwrap_or_else(empty_metrics);
                (m, Vec::new())
            } else {
                let prepared = prepared_cache
                    .iter()
                    .find(|(c, _)| *c == config)
                    .map(|(_, p)| Arc::clone(p));
                let Some(prepared) = prepared else { continue };
                self.trial_scorer
                    .score_variant(effect.run_id, &prepared, &validation_subset, &options)
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
                        variant_config: config,
                        options,
                        split: EvaluationResultSplit::Validation,
                        chunk_set_id,
                        embedding_set_id,
                        metrics,
                        retrieval_traces: traces,
                        selected: false,
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
        }

        let champion_tid = validation_scores
            .iter()
            .max_by(|a, b| {
                evaluation_score(a.1)
                    .partial_cmp(&evaluation_score(b.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(tid, _)| *tid);

        let champion: Option<(u32, EvaluationMetrics)> = match champion_tid {
            Some(trial_id) => {
                let config = last_config.get(&trial_id).copied();
                let options = last_options.get(&trial_id).cloned();
                let chunk_set_id = last_chunk_set.get(&trial_id).copied();
                let embedding_set_id = last_embedding_set.get(&trial_id).copied();
                let prepared = config.and_then(|c| {
                    prepared_cache
                        .iter()
                        .find(|(cc, _)| *cc == c)
                        .map(|(_, p)| Arc::clone(p))
                });

                match (config, options, chunk_set_id, embedding_set_id, prepared) {
                    (
                        Some(config),
                        Some(options),
                        Some(chunk_set_id),
                        Some(embedding_set_id),
                        Some(prepared),
                    ) => {
                        job.emit(
                            InternalLogEvent::info(format!(
                                "Holdout integrity pass: scoring champion (trial {trial_id}) on {} holdout questions",
                                split.holdout.len(),
                            ))
                            .with_meta("trial_id", json!(trial_id))
                            .with_meta("holdout_question_count", json!(split.holdout.len())),
                        )
                        .await;

                        let already_scored = resume.scored_holdout.contains(&trial_id);
                        let (metrics, traces) = if already_scored {
                            let m = resume
                                .holdout_metrics
                                .get(&trial_id)
                                .cloned()
                                .unwrap_or_else(empty_metrics);
                            (m, Vec::new())
                        } else {
                            self.trial_scorer
                                .score_variant(effect.run_id, &prepared, &holdout_subset, &options)
                                .await?
                        };

                        if !already_scored {
                            self.command_processor
                                .handle(
                                    effect.run_id,
                                    EvaluationRunCommand::ScoreVariant(ScoreVariant {
                                        run_id: effect.run_id,
                                        variant_label: encoding::trial_holdout_label(trial_id),
                                        variant_config: config,
                                        options,
                                        split: EvaluationResultSplit::Holdout,
                                        chunk_set_id,
                                        embedding_set_id,
                                        metrics: metrics.clone(),
                                        retrieval_traces: traces,
                                        selected: true,
                                        occurred_at: self.clock.now(),
                                    }),
                                )
                                .await?;
                        }

                        Some((trial_id, metrics))
                    }
                    _ => None,
                }
            }
            None => None,
        };

        if let Some((trial_id, metrics)) = champion {
            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::SelectChampion(SelectChampion {
                        run_id: effect.run_id,
                        trial_id,
                        holdout_metrics: metrics,
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
                "Optimization complete · {} trials across {} rungs",
                last_metrics.len(),
                schedule.len(),
            ))
            .with_meta("run_id", json!(effect.run_id.to_string()))
            .with_meta("trial_count", json!(last_metrics.len())),
        )
        .await;

        Ok(())
    }

    async fn load_resume_state(&self, run_id: Uuid) -> Result<ResumeState, AppError> {
        let envelopes = self.event_store.load(run_id).await?;
        let mut trials: HashMap<u32, (HashMap<String, Value>, u32)> = HashMap::new();
        let mut scored_tuning: HashSet<(u32, u32)> = HashSet::new();
        let mut scored_validation: HashSet<u32> = HashSet::new();
        let mut scored_holdout: HashSet<u32> = HashSet::new();
        let mut scored_metrics: HashMap<(u32, u32), EvaluationMetrics> = HashMap::new();
        let mut final_rung_obs: HashMap<u32, Observation> = HashMap::new();
        let mut highest_seen_rung: HashMap<u32, u32> = HashMap::new();
        let mut validation_metrics: HashMap<u32, EvaluationMetrics> = HashMap::new();
        let mut holdout_metrics: HashMap<u32, EvaluationMetrics> = HashMap::new();
        let mut rung_survivors: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut has_champion = false;

        for env in &envelopes {
            match &env.event {
                EvaluationRunEvent::TrialProposed(t) => {
                    let params = encoding::params_from_json(&t.params);
                    trials.insert(t.trial_id, (params, t.rung));
                }
                EvaluationRunEvent::VariantScored(s) => {
                    if let Some((trial_id, rung)) =
                        encoding::parse_trial_rung_label(&s.variant_label)
                    {
                        scored_tuning.insert((trial_id, rung));

                        scored_metrics.insert((trial_id, rung), s.metrics.clone());
                        let params = trials
                            .get(&trial_id)
                            .map(|(p, _)| p.clone())
                            .unwrap_or_default();
                        let fitness = Fitness {
                            composite: evaluation_score(&s.metrics),
                            composite_ci: (s.metrics.composite_ci_low, s.metrics.composite_ci_high),
                            recall: s.metrics.recall_mean,
                            precision: s.metrics.precision_mean,
                            iou: s.metrics.iou_mean,
                            precision_omega: s.metrics.precision_omega_mean,
                            cost: s.metrics.average_retrieved_tokens as f32,
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
                        scored_validation.insert(trial_id);
                        validation_metrics.insert(trial_id, s.metrics.clone());
                    } else if let Some(trial_id) =
                        encoding::parse_trial_holdout_label(&s.variant_label)
                    {
                        scored_holdout.insert(trial_id);
                        holdout_metrics.insert(trial_id, s.metrics.clone());
                    }
                }
                EvaluationRunEvent::RungAdvanced(r) => {
                    rung_survivors.insert(r.rung, r.surviving_trials.clone());
                }
                EvaluationRunEvent::ChampionSelected(_) => has_champion = true,
                _ => {}
            }
        }

        Ok(ResumeState {
            trials,
            scored_tuning,
            scored_validation,
            scored_holdout,
            scored_metrics,
            final_rung_obs,
            validation_metrics,
            holdout_metrics,
            rung_survivors,
            has_champion,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_judge_pass(
        &self,
        effect: &OptimizeRunEffect,
        job: &Arc<Job>,
        ctx: &RunContext,
        prepared_cache: &[(ChunkingConfig, Arc<PreparedVariant>)],
        final_scored: &[(u32, f32)],
        last_config: &HashMap<u32, ChunkingConfig>,
        last_options: &HashMap<u32, EvaluationRunOptions>,
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
            let Some(config) = last_config.get(trial_id).copied() else {
                continue;
            };
            let Some(options) = last_options.get(trial_id).cloned() else {
                continue;
            };
            let prepared = prepared_cache
                .iter()
                .find(|(c, _)| *c == config)
                .map(|(_, p)| Arc::clone(p));
            let Some(prepared) = prepared else { continue };

            let mut accum = 0.0f32;
            let mut counted = 0usize;
            for &q_idx in &sample {
                let question = &ctx.questions[q_idx];
                let q_emb = &ctx.question_embeddings[q_idx];
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
                    InternalLogEvent::info(format!("Trial {trial_id} judge score = {:.2}", score))
                        .with_meta("trial_id", json!(trial_id))
                        .with_meta("judge_score", json!(score)),
                )
                .await;
            }
        }
        judge_scores
    }

    async fn ensure_prepared(
        &self,
        ctx: &RunContext,
        cache: &mut Vec<(ChunkingConfig, Arc<PreparedVariant>)>,
        config: ChunkingConfig,
        trial_id: u32,
    ) -> Result<Arc<PreparedVariant>, AppError> {
        if let Some((_, existing)) = cache.iter().find(|(c, _)| *c == config) {
            return Ok(Arc::clone(existing));
        }
        let label = encoding::trial_label(trial_id);
        let prepared = self
            .trial_scorer
            .prepare_variant(ctx, label, config)
            .await?;
        let arc = Arc::new(prepared);
        cache.push((config, Arc::clone(&arc)));
        Ok(arc)
    }
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

fn empty_metrics() -> EvaluationMetrics {
    EvaluationMetrics {
        recall_mean: 0.0,
        recall_std: 0.0,
        precision_mean: 0.0,
        precision_std: 0.0,
        iou_mean: 0.0,
        iou_std: 0.0,
        precision_omega_mean: 0.0,
        precision_omega_std: 0.0,
        chunk_count: 0,
        average_chunk_tokens: 0,
        average_retrieved_tokens: 0,
        recall_ci_low: 0.0,
        recall_ci_high: 0.0,
        precision_ci_low: 0.0,
        precision_ci_high: 0.0,
        iou_ci_low: 0.0,
        iou_ci_high: 0.0,
        precision_omega_ci_low: 0.0,
        precision_omega_ci_high: 0.0,
        composite_ci_low: 0.0,
        composite_ci_high: 0.0,
        judge_score: None,
    }
}

fn shuffled_tuning_order(tuning: &[usize], run_id: Uuid) -> Vec<usize> {
    let mut indices: Vec<usize> = tuning.to_vec();
    let mut state = run_id.as_u128() as u64;
    if state == 0 {
        state = 0xCBF2_9CE4_8422_2325;
    }
    for i in (1..indices.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        indices.swap(i, j);
    }
    indices
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
    rung: crate::server::domain::evaluation::optimizer::Rung,
) -> QuestionSubset<'a> {
    let take = rung
        .question_count(tuning_order.len())
        .min(tuning_order.len());
    let picked: &[usize] = &tuning_order[..take];
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

fn uuid_to_seed(run_id: Uuid) -> u64 {
    let bytes = run_id.as_bytes();
    let mut hi = 0u64;
    let mut lo = 0u64;
    for i in 0..8 {
        hi = (hi << 8) | bytes[i] as u64;
        lo = (lo << 8) | bytes[8 + i] as u64;
    }
    hi ^ lo
}
