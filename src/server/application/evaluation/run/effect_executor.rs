use std::collections::BTreeSet;
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
use crate::server::domain::evaluation::run::effects::{CompleteRunEffect, ExecuteVariantEffect};
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::run::repository::EvaluationRunRepository;
use crate::shared::{
    ChunkingConfig, EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
};
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::AggregateRepository;

const COMPLETE_SESSION_ID_NAMESPACE: Uuid = uuid::uuid!("c5d6f3c0-1a3a-4d36-9f2e-2cb1f7d6a801");

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
        let variant_config: ChunkingConfig = effect.variant_config.into();
        let options_shared: Vec<EvaluationRunOptions> =
            effect.options.iter().cloned().map(Into::into).collect();

        let ctx = self
            .trial_scorer
            .load_run_context(effect.dataset_id, effect.index_profile_id)
            .await?;

        job.emit(
            InternalLogEvent::info(format!("Preparing variant '{}'…", effect.variant_label))
                .with_meta("variant_label", json!(effect.variant_label)),
        )
        .await;

        let prepared = self
            .trial_scorer
            .prepare_variant(&ctx, effect.variant_label.clone(), variant_config)
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

        let full_indices: Vec<usize> = (0..ctx.questions.len()).collect();
        let subset =
            QuestionSubset::from_indices(&ctx.questions, &ctx.question_embeddings, &full_indices);

        let already_scored = self.scored_keys_for_variant(effect.run_id).await?;

        for options in &options_shared {
            let key = ScoredVariantKey {
                variant_label: effect.variant_label.clone(),
                options: options.clone().into(),
                split: EvaluationResultSplit::Full.into(),
            };
            if already_scored.contains(&key) {
                continue;
            }
            let (metrics, traces) = self
                .trial_scorer
                .score_variant(effect.run_id, &prepared, &subset, options)
                .await?;
            self.record_scored(effect.run_id, &job, &prepared, options, metrics, traces)
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

    async fn record_scored(
        &self,
        run_id: Uuid,
        job: &Arc<Job>,
        variant: &PreparedVariant,
        options: &EvaluationRunOptions,
        metrics: EvaluationMetrics,
        retrieval_traces: Vec<RetrievalTraceEntry>,
    ) -> Result<(), AppError> {
        log_scored(job, &variant.label, options, &metrics).await;
        self.command_processor
            .handle(
                run_id,
                EvaluationRunCommand::ScoreVariant(ScoreVariant {
                    run_id,
                    variant_label: variant.label.clone(),
                    variant_config: variant.config.into(),
                    options: options.clone().into(),
                    split: EvaluationResultSplit::Full.into(),
                    chunk_set_id: variant.chunk_set_id,
                    embedding_set_id: variant.embedding_set_id,
                    metrics: metrics.into(),
                    retrieval_traces,
                    selected: false,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }
}

pub struct CompleteRunEffectExecutor {
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    run_repository: Arc<dyn EvaluationRunRepository>,
    session: JobSession<EvaluationRun>,
    clock: Arc<dyn Clock>,
}

impl CompleteRunEffectExecutor {
    pub fn new(
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
            command_processor,
            run_repository,
            session,
            clock,
        })
    }

    pub(crate) async fn run(&self, effect: &CompleteRunEffect) -> Result<(), AppError> {
        let run_id = effect.run_id;
        let clock_for_fail = Arc::clone(&self.clock);
        self.session
            .run(
                Uuid::new_v5(&COMPLETE_SESSION_ID_NAMESPACE, run_id.as_bytes()),
                JobIdStrategy::Fresh,
                "Evaluation completion failed",
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

    async fn run_inner(&self, effect: &CompleteRunEffect, job: Arc<Job>) -> Result<(), AppError> {
        let run = self
            .run_repository
            .load(effect.run_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("run {} not found", effect.run_id)))?;

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
        job.emit(
            InternalLogEvent::success(format!(
                "Evaluation run complete · {variant_count} variants × {option_count} options scored",
            ))
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
    metrics: &EvaluationMetrics,
) {
    job.emit(
        InternalLogEvent::info(format!(
            "Scored variant '{}' (top_k={}): recall={:.3} precision={:.3} iou={:.3}",
            variant_label,
            options.top_k,
            metrics.recall_mean,
            metrics.precision_mean,
            metrics.iou_mean,
        ))
        .with_meta("variant_label", json!(variant_label))
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
