use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::core::{
    evaluation_score, EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
};
use crate::server::application::evaluation::scoring::{
    PreparedVariant, QuestionSubset, RunContext, TrialScorer,
};
use crate::server::application::ports::Clock;
use crate::server::application::{
    ActivityRegistry, AppError, InternalLogEvent, Job, JobIdStrategy, JobRegistry, JobSession,
};
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{
    CompleteRun, EvaluationRunCommand, FailRun, MarkVariantPrepared, ScoreVariant,
};
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::split::{split_questions, DatasetSplit};
use crate::server::event_sourcing::command_processor::CommandProcessor;

use crate::server::domain::evaluation::run::effects::ExecuteRunEffect;

pub struct EvaluationRunEffectExecutor {
    trial_scorer: Arc<TrialScorer>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    session: JobSession<EvaluationRun>,
    clock: Arc<dyn Clock>,
}
struct ScoredCombo {
    variant_index: usize,
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
    fn for_run(effect: &ExecuteRunEffect, question_count: usize) -> Result<Self, AppError> {
        match &effect.autotune_request {
            Some(req) => {
                let split =
                    split_questions(effect.run_id, question_count, req.tuning_fraction_milli);
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

    fn is_autotune(&self) -> bool {
        !self.holdout_indices.is_empty()
    }
}

impl EvaluationRunEffectExecutor {
    pub fn new(
        trial_scorer: Arc<TrialScorer>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
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
            session,
            clock,
        })
    }

    pub(crate) async fn run(&self, effect: &ExecuteRunEffect) -> Result<(), AppError> {
        let run_id = effect.run_id;
        let clock_for_fail = Arc::clone(&self.clock);
        self.session
            .run(
                run_id,
                JobIdStrategy::Fresh,
                "Evaluation run failed",
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

    async fn run_inner(&self, effect: &ExecuteRunEffect, job: Arc<Job>) -> Result<(), AppError> {
        let ctx = self
            .trial_scorer
            .load_run_context(effect.dataset_id, effect.pipeline_configuration_id)
            .await?;
        let plan = EvaluationPlan::for_run(effect, ctx.questions.len())?;

        self.emit_start_log(&job, effect, &ctx, &plan).await;

        let prepared = self.prepare_variants(effect, &job, &ctx).await?;

        let primary = QuestionSubset::from_indices(
            &ctx.questions,
            &ctx.question_embeddings,
            &plan.primary_indices,
        );
        let primary_results = self
            .score_grid(
                effect.run_id,
                &job,
                &prepared,
                &effect.options,
                &primary,
                plan.primary_split,
            )
            .await?;

        if plan.is_autotune() {
            let leaders = top_n(&primary_results, plan.holdout_top_n);
            let holdout = QuestionSubset::from_indices(
                &ctx.questions,
                &ctx.question_embeddings,
                &plan.holdout_indices,
            );

            job.emit(
                InternalLogEvent::info(format!(
                    "Tuning complete · scoring top {n} candidate{plural} on holdout",
                    n = leaders.len(),
                    plural = if leaders.len() == 1 { "" } else { "s" },
                ))
                .with_meta("top_n", json!(leaders.len())),
            )
            .await;

            self.score_candidates(
                effect.run_id,
                &job,
                &prepared,
                &leaders,
                &holdout,
                EvaluationResultSplit::Holdout,
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

        self.emit_complete_log(&job, effect, &plan).await;
        Ok(())
    }

    async fn prepare_variants(
        &self,
        effect: &ExecuteRunEffect,
        job: &Arc<Job>,
        ctx: &RunContext,
    ) -> Result<Vec<PreparedVariant>, AppError> {
        let mut out = Vec::with_capacity(effect.variants.len());
        for variant in &effect.variants {
            job.emit(
                InternalLogEvent::info(format!("Preparing variant '{}'…", variant.label))
                    .with_meta("variant_label", json!(variant.label)),
            )
            .await;

            let prepared = self
                .trial_scorer
                .prepare_variant(ctx, variant.label.clone(), variant.config)
                .await?;

            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                        run_id: effect.run_id,
                        variant_label: variant.label.clone(),
                        chunk_set_id: prepared.chunk_set_id,
                        embedding_set_id: prepared.embedding_set_id,
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;

            job.emit(
                InternalLogEvent::info(format!(
                    "Variant '{}' prepared: {} chunks",
                    variant.label,
                    prepared.chunks.len(),
                ))
                .with_meta("variant_label", json!(variant.label))
                .with_meta("chunk_count", json!(prepared.chunks.len()))
                .with_meta("chunk_set_id", json!(prepared.chunk_set_id.to_string()))
                .with_meta(
                    "embedding_set_id",
                    json!(prepared.embedding_set_id.to_string()),
                ),
            )
            .await;

            out.push(prepared);
        }
        Ok(out)
    }

    async fn score_grid(
        &self,
        run_id: Uuid,
        job: &Arc<Job>,
        prepared: &[PreparedVariant],
        options_grid: &[EvaluationRunOptions],
        subset: &QuestionSubset<'_>,
        split: EvaluationResultSplit,
    ) -> Result<Vec<ScoredCombo>, AppError> {
        let mut scored = Vec::with_capacity(prepared.len() * options_grid.len());
        for (variant_index, variant) in prepared.iter().enumerate() {
            for options in options_grid {
                let (metrics, traces) = self
                    .trial_scorer
                    .score_variant(run_id, variant, subset, options)
                    .await?;
                let score = evaluation_score(&metrics);
                scored.push(ScoredCombo {
                    variant_index,
                    options: options.clone(),
                    score,
                });
                self.record_scored(run_id, job, variant, options, split, metrics, traces, false)
                    .await?;
            }
        }
        Ok(scored)
    }

    async fn score_candidates(
        &self,
        run_id: Uuid,
        job: &Arc<Job>,
        prepared: &[PreparedVariant],
        candidates: &[&ScoredCombo],
        subset: &QuestionSubset<'_>,
        split: EvaluationResultSplit,
    ) -> Result<(), AppError> {
        for (rank, combo) in candidates.iter().enumerate() {
            let Some(variant) = prepared.get(combo.variant_index) else {
                continue;
            };
            let (metrics, traces) = self
                .trial_scorer
                .score_variant(run_id, variant, subset, &combo.options)
                .await?;
            self.record_scored(
                run_id,
                job,
                variant,
                &combo.options,
                split,
                metrics,
                traces,
                rank == 0,
            )
            .await?;
        }
        Ok(())
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
                    variant_config: variant.config,
                    options: options.clone(),
                    split,
                    chunk_set_id: variant.chunk_set_id,
                    embedding_set_id: variant.embedding_set_id,
                    metrics,
                    retrieval_traces,
                    selected,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }

    async fn emit_start_log(
        &self,
        job: &Arc<Job>,
        effect: &ExecuteRunEffect,
        ctx: &RunContext,
        plan: &EvaluationPlan,
    ) {
        let message = if plan.is_autotune() {
            format!(
                "Starting autotune: {} variants × {} options · {} tuning / {} holdout questions ({})",
                effect.variants.len(),
                effect.options.len(),
                plan.primary_indices.len(),
                plan.holdout_indices.len(),
                ctx.embedding_model.model,
            )
        } else {
            format!(
                "Starting evaluation run: {} variants × {} options across {} questions ({})",
                effect.variants.len(),
                effect.options.len(),
                ctx.questions.len(),
                ctx.embedding_model.model,
            )
        };
        let mut log = InternalLogEvent::info(message)
            .with_meta("run_id", json!(effect.run_id.to_string()))
            .with_meta("dataset_id", json!(effect.dataset_id.to_string()))
            .with_meta("variant_count", json!(effect.variants.len()))
            .with_meta("option_count", json!(effect.options.len()))
            .with_meta("embedding_model", json!(ctx.embedding_model.model));
        if plan.is_autotune() {
            log = log
                .with_meta("tuning_question_count", json!(plan.primary_indices.len()))
                .with_meta("holdout_question_count", json!(plan.holdout_indices.len()));
        } else {
            log = log.with_meta("question_count", json!(ctx.questions.len()));
        }
        job.emit(log).await;
    }

    async fn emit_complete_log(
        &self,
        job: &Arc<Job>,
        effect: &ExecuteRunEffect,
        plan: &EvaluationPlan,
    ) {
        let message = if plan.is_autotune() {
            let actual_holdout = plan
                .holdout_top_n
                .min(effect.variants.len() * effect.options.len());
            format!(
                "Autotune complete · {} variants × {} options on tuning, top {} on holdout",
                effect.variants.len(),
                effect.options.len(),
                actual_holdout,
            )
        } else {
            format!(
                "Evaluation run complete · {} variants × {} options scored",
                effect.variants.len(),
                effect.options.len(),
            )
        };
        job.emit(
            InternalLogEvent::success(message)
                .with_meta("run_id", json!(effect.run_id.to_string()))
                .with_meta("variant_count", json!(effect.variants.len()))
                .with_meta("option_count", json!(effect.options.len())),
        )
        .await;
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
            .unwrap_or(std::cmp::Ordering::Equal)
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
            variant_index: 0,
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
        assert_eq!(leaders[0].variant_index, 0);
    }
}
