use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use super::scoring::{PreparedVariant, QuestionSubset, TrialScorer};
use crate::server::application::ports::Clock;
use crate::server::application::{
    ActivityRegistry, AppError, InternalLogEvent, Job, JobIdStrategy, JobRegistry, JobSession,
};
use crate::server::domain::evaluation::run::aggregate::{EvaluationRun, ScoredVariantKey};
use crate::server::domain::evaluation::run::commands::{
    CompleteRun, EvaluationRunCommand, FailRun, MarkVariantPrepared, ScoreVariant,
};
use crate::server::domain::evaluation::run::effects::{ExecuteVariantEffect, FinalizeRunEffect};
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::run::repository::EvaluationRunRepository;
use crate::server::domain::evaluation::split::{seed_from_uuid, split_questions, DatasetSplit};
use crate::server::domain::evaluation::value_objects::EvaluationResultSplit as DomainEvaluationResultSplit;
use crate::shared::{
    evaluation_score, ChunkingConfig, EvaluationAutotuneRequest as EvaluationAutotuneRequestDto,
    EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
};
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::AggregateRepository;

const FINALIZE_SESSION_ID_NAMESPACE: Uuid = uuid::uuid!("c5d6f3c0-1a3a-4d36-9f2e-2cb1f7d6a801");

struct ScoredCombo {
    variant_label: String,
    variant_config: ChunkingConfig,
    chunk_set_id: Uuid,
    embedding_set_id: Uuid,
    options: EvaluationRunOptions,
    score: f32,
}

struct EvaluationPlan {
    primary_split: EvaluationResultSplit,
    primary_indices: Vec<usize>,
    holdout_indices: Vec<usize>,
    holdout_top_n: usize,
}

impl EvaluationPlan {
    fn for_run(
        run_id: Uuid,
        autotune: Option<&EvaluationAutotuneRequestDto>,
        question_count: usize,
    ) -> Result<Self, AppError> {
        match autotune {
            Some(req) => {
                let split = split_questions(
                    seed_from_uuid(run_id),
                    question_count,
                    req.tuning_fraction_milli,
                );
                if !split.is_usable() {
                    return Err(AppError::Validation(format!(
                        "autotune needs at least 2 questions for a {:.0}/{:.0} split (got {question_count})",
                        req.tuning_fraction() * 100.0,
                        (1.0 - req.tuning_fraction()) * 100.0,
                    )));
                }
                let DatasetSplit { tuning, holdout } = split;
                Ok(Self {
                    primary_split: EvaluationResultSplit::Tuning,
                    primary_indices: tuning,
                    holdout_indices: holdout,
                    holdout_top_n: req.holdout_top_n as usize,
                })
            }
            None => Ok(Self {
                primary_split: EvaluationResultSplit::Full,
                primary_indices: (0..question_count).collect(),
                holdout_indices: Vec::new(),
                holdout_top_n: 0,
            }),
        }
    }
}

pub struct ExecuteVariantEffectExecutor {
    trial_scorer: Arc<TrialScorer>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    aggregate_repository: Arc<AggregateRepository<EvaluationRun>>,
    session: JobSession<EvaluationRun>,
    clock: Arc<dyn Clock>,
}

impl ExecuteVariantEffectExecutor {
    pub fn new(
        trial_scorer: Arc<TrialScorer>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        aggregate_repository: Arc<AggregateRepository<EvaluationRun>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
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
            aggregate_repository,
            session,
            clock,
        })
    }

    pub(crate) async fn run(&self, effect: &ExecuteVariantEffect) -> Result<(), AppError> {
        let run_id = effect.run_id;
        let clock_for_fail = Arc::clone(&self.clock);
        self.session
            .run(
                run_id,
                JobIdStrategy::Fresh,
                "Evaluation variant failed",
                move |reason| {
                    EvaluationRunCommand::FailRun(FailRun {
                        run_id,
                        reason,
                        occurred_at: clock_for_fail.now(),
                    })
                },
                |job| self.run_inner(effect, job),
            )
            .await
    }

    async fn run_inner(
        &self,
        effect: &ExecuteVariantEffect,
        job: Arc<Job>,
    ) -> Result<(), AppError> {
        let variant_config_shared: ChunkingConfig = effect.variant_config.into();
        let options_shared: Vec<EvaluationRunOptions> =
            effect.options.iter().cloned().map(Into::into).collect();
        let autotune_shared: Option<EvaluationAutotuneRequestDto> =
            effect.autotune_request.clone().map(Into::into);

        let ctx = self
            .trial_scorer
            .load_run_context(effect.dataset_id, effect.pipeline_configuration_id)
            .await?;
        let plan =
            EvaluationPlan::for_run(effect.run_id, autotune_shared.as_ref(), ctx.questions.len())?;

        job.emit(
            InternalLogEvent::info(format!("Preparing variant '{}'…", effect.variant_label))
                .with_meta("variant_label", json!(effect.variant_label)),
        )
        .await;

        let prepared = self
            .trial_scorer
            .prepare_variant(&ctx, effect.variant_label.clone(), variant_config_shared)
            .await?;

        self.command_processor
            .handle(
                effect.run_id,
                EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                    run_id: effect.run_id,
                    variant_label: effect.variant_label.clone(),
                    chunk_set_id: prepared.chunk_set_id,
                    embedding_set_id: prepared.embedding_set_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        job.emit(
            InternalLogEvent::info(format!(
                "Variant '{}' prepared: {} chunks",
                effect.variant_label,
                prepared.chunks.len(),
            ))
            .with_meta("variant_label", json!(effect.variant_label))
            .with_meta("chunk_count", json!(prepared.chunks.len()))
            .with_meta("chunk_set_id", json!(prepared.chunk_set_id.to_string()))
            .with_meta(
                "embedding_set_id",
                json!(prepared.embedding_set_id.to_string()),
            ),
        )
        .await;

        let primary = QuestionSubset::from_indices(
            &ctx.questions,
            &ctx.question_embeddings,
            &plan.primary_indices,
        );

        let already_scored = self.scored_keys_for_variant(effect.run_id).await?;

        for options in &options_shared {
            let key = ScoredVariantKey {
                variant_label: effect.variant_label.clone(),
                options: options.clone().into(),
                split: plan.primary_split.into(),
            };
            if already_scored.contains(&key) {
                continue;
            }
            let (metrics, traces) = self
                .trial_scorer
                .score_variant(effect.run_id, &prepared, &primary, options)
                .await?;
            self.record_scored(
                effect.run_id,
                &job,
                &prepared,
                options,
                plan.primary_split,
                metrics,
                traces,
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn scored_keys_for_variant(
        &self,
        run_id: Uuid,
    ) -> Result<BTreeSet<ScoredVariantKey>, AppError> {
        match self.aggregate_repository.load(run_id).await? {
            Some(loaded) => Ok(loaded.aggregate.scored_keys),
            None => Ok(BTreeSet::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_scored(
        &self,
        run_id: Uuid,
        job: &Arc<Job>,
        variant: &PreparedVariant,
        options: &EvaluationRunOptions,
        split: EvaluationResultSplit,
        metrics: EvaluationMetrics,
        retrieval_traces: Vec<RetrievalTraceEntry>,
        selected: bool,
    ) -> Result<(), AppError> {
        log_scored(job, &variant.label, options, split, &metrics).await;
        self.command_processor
            .handle(
                run_id,
                EvaluationRunCommand::ScoreVariant(ScoreVariant {
                    run_id,
                    variant_label: variant.label.clone(),
                    variant_config: variant.config.into(),
                    options: options.clone().into(),
                    split: split.into(),
                    chunk_set_id: variant.chunk_set_id,
                    embedding_set_id: variant.embedding_set_id,
                    metrics: metrics.into(),
                    retrieval_traces,
                    selected,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }
}

pub struct FinalizeRunEffectExecutor {
    trial_scorer: Arc<TrialScorer>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    run_repository: Arc<dyn EvaluationRunRepository>,
    session: JobSession<EvaluationRun>,
    clock: Arc<dyn Clock>,
}

impl FinalizeRunEffectExecutor {
    pub fn new(
        trial_scorer: Arc<TrialScorer>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        run_repository: Arc<dyn EvaluationRunRepository>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
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
            run_repository,
            session,
            clock,
        })
    }

    pub(crate) async fn run(&self, effect: &FinalizeRunEffect) -> Result<(), AppError> {
        let run_id = effect.run_id;
        let clock_for_fail = Arc::clone(&self.clock);
        self.session
            .run(
                Uuid::new_v5(&FINALIZE_SESSION_ID_NAMESPACE, run_id.as_bytes()),
                JobIdStrategy::Fresh,
                "Evaluation finalize failed",
                move |reason| {
                    EvaluationRunCommand::FailRun(FailRun {
                        run_id,
                        reason,
                        occurred_at: clock_for_fail.now(),
                    })
                },
                |job| self.run_inner(effect, job),
            )
            .await
    }

    async fn run_inner(&self, effect: &FinalizeRunEffect, job: Arc<Job>) -> Result<(), AppError> {
        let run = self
            .run_repository
            .load(effect.run_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("run {} not found", effect.run_id)))?;
        let variant_results = self
            .run_repository
            .load_variant_results(effect.run_id)
            .await?;

        if effect.autotune_request.is_some() {
            let autotune_shared: Option<EvaluationAutotuneRequestDto> =
                effect.autotune_request.clone().map(Into::into);
            let ctx = self
                .trial_scorer
                .load_run_context(effect.dataset_id, effect.pipeline_configuration_id)
                .await?;
            let plan = EvaluationPlan::for_run(
                effect.run_id,
                autotune_shared.as_ref(),
                ctx.questions.len(),
            )?;

            let primary_split_domain: DomainEvaluationResultSplit = plan.primary_split.into();
            let primary_combos: Vec<ScoredCombo> = variant_results
                .iter()
                .filter(|r| r.split == primary_split_domain)
                .map(|r| {
                    let metrics_dto: EvaluationMetrics = r.metrics().into();
                    ScoredCombo {
                        variant_label: r.variant_label.clone(),
                        variant_config: r.variant_config.into(),
                        chunk_set_id: r.chunk_set_id,
                        embedding_set_id: r.embedding_set_id,
                        options: r.options.clone().into(),
                        score: evaluation_score(&metrics_dto),
                    }
                })
                .collect();
            let leaders = top_n(&primary_combos, plan.holdout_top_n);

            job.emit(
                InternalLogEvent::info(format!(
                    "Tuning complete · scoring top {n} candidate{plural} on holdout",
                    n = leaders.len(),
                    plural = if leaders.len() == 1 { "" } else { "s" },
                ))
                .with_meta("top_n", json!(leaders.len())),
            )
            .await;

            let holdout_subset = QuestionSubset::from_indices(
                &ctx.questions,
                &ctx.question_embeddings,
                &plan.holdout_indices,
            );

            let holdout_split_domain: DomainEvaluationResultSplit =
                EvaluationResultSplit::Holdout.into();
            let already_scored: BTreeSet<_> = variant_results
                .iter()
                .filter(|r| r.split == holdout_split_domain)
                .map(|r| {
                    let opts: EvaluationRunOptions = r.options.clone().into();
                    (r.variant_label.clone(), opts)
                })
                .collect();

            for (rank, combo) in leaders.iter().enumerate() {
                if already_scored.contains(&(combo.variant_label.clone(), combo.options.clone())) {
                    continue;
                }
                let prepared = PreparedVariant {
                    label: combo.variant_label.clone(),
                    config: combo.variant_config,
                    chunk_set_id: combo.chunk_set_id,
                    embedding_set_id: combo.embedding_set_id,
                    chunks: Vec::new(),
                    chunk_token_counts: HashMap::new(),
                };
                let (metrics, traces) = self
                    .trial_scorer
                    .score_variant(effect.run_id, &prepared, &holdout_subset, &combo.options)
                    .await?;
                log_scored(
                    &job,
                    &combo.variant_label,
                    &combo.options,
                    EvaluationResultSplit::Holdout,
                    &metrics,
                )
                .await;
                self.command_processor
                    .handle(
                        effect.run_id,
                        EvaluationRunCommand::ScoreVariant(ScoreVariant {
                            run_id: effect.run_id,
                            variant_label: combo.variant_label.clone(),
                            variant_config: combo.variant_config.into(),
                            options: combo.options.clone().into(),
                            split: EvaluationResultSplit::Holdout.into(),
                            chunk_set_id: combo.chunk_set_id,
                            embedding_set_id: combo.embedding_set_id,
                            metrics: metrics.into(),
                            retrieval_traces: traces,
                            selected: rank == 0,
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
            }
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

        let variant_count = run.variants.len();
        let option_count = run.options.len();
        let message = if effect.autotune_request.is_some() {
            format!(
                "Autotune complete · {variant_count} variants × {option_count} options on tuning",
            )
        } else {
            format!(
                "Evaluation run complete · {variant_count} variants × {option_count} options scored",
            )
        };
        job.emit(
            InternalLogEvent::success(message)
                .with_meta("run_id", json!(effect.run_id.to_string()))
                .with_meta("variant_count", json!(variant_count))
                .with_meta("option_count", json!(option_count)),
        )
        .await;
        Ok(())
    }
}

async fn log_scored(
    job: &Arc<Job>,
    variant_label: &str,
    options: &EvaluationRunOptions,
    split: EvaluationResultSplit,
    metrics: &EvaluationMetrics,
) {
    job.emit(
        InternalLogEvent::info(format!(
            "Scored variant '{}' (top_k={}, split={}): recall={:.3} precision={:.3} iou={:.3}",
            variant_label,
            options.top_k,
            split.as_str(),
            metrics.recall_mean,
            metrics.precision_mean,
            metrics.iou_mean,
        ))
        .with_meta("variant_label", json!(variant_label))
        .with_meta("split", json!(split.as_str()))
        .with_meta("top_k", json!(options.top_k))
        .with_meta("min_score_milli", json!(options.min_score_milli))
        .with_meta("recall_mean", json!(metrics.recall_mean))
        .with_meta("recall_std", json!(metrics.recall_std))
        .with_meta("precision_mean", json!(metrics.precision_mean))
        .with_meta("precision_std", json!(metrics.precision_std))
        .with_meta("iou_mean", json!(metrics.iou_mean))
        .with_meta("iou_std", json!(metrics.iou_std))
        .with_meta("precision_omega_mean", json!(metrics.precision_omega_mean)),
    )
    .await;
}

fn top_n(scored: &[ScoredCombo], n: usize) -> Vec<&ScoredCombo> {
    let mut indexed: Vec<(usize, &ScoredCombo)> = scored.iter().enumerate().collect();
    indexed.sort_by(|(ai, a), (bi, b)| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then(ai.cmp(bi))
    });
    indexed
        .into_iter()
        .take(n.min(scored.len()))
        .map(|(_, c)| c)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(score: f32) -> ScoredCombo {
        ScoredCombo {
            variant_label: "v".to_string(),
            variant_config: ChunkingConfig::default(),
            chunk_set_id: Uuid::nil(),
            embedding_set_id: Uuid::nil(),
            options: EvaluationRunOptions::default(),
            score,
        }
    }

    #[test]
    fn top_n_returns_highest_scores_first() {
        let combos = vec![combo(0.1), combo(0.9), combo(0.5)];
        let leaders = top_n(&combos, 2);
        assert_eq!(leaders.len(), 2);
        assert!((leaders[0].score - 0.9).abs() < 1e-6);
        assert!((leaders[1].score - 0.5).abs() < 1e-6);
    }

    #[test]
    fn top_n_caps_at_input_length() {
        let combos = vec![combo(0.5)];
        let leaders = top_n(&combos, 5);
        assert_eq!(leaders.len(), 1);
    }

    #[test]
    fn top_n_breaks_ties_by_input_order() {
        let combos = vec![combo(0.5), combo(0.5)];
        let leaders = top_n(&combos, 1);
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].variant_label, "v");
    }
}
