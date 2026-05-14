use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::generator::build_question_prompt;
use crate::server::application::evaluation::ports::EvaluationGenerator;
use crate::server::application::evaluation::question_filter::{
    GeneratedQuestionGate, QuestionFilterDecision,
};
use crate::server::application::evaluation::reference_locator::ReferenceLocator;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{ActivityRegistry, AppError, InternalLogEvent, JobRegistry};
use crate::server::domain::evaluation::dataset::aggregate::EvaluationDataset;
use crate::server::domain::evaluation::dataset::commands::{
    AcceptQuestion, CompleteDatasetGeneration, EvaluationDatasetCommand, FailDatasetGeneration,
    RejectQuestion,
};
use crate::server::domain::evaluation::question::EvaluationReference;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::process_manager::EffectExecutor;
use crate::shared::plain_f32_vec;

use crate::server::application::ports::Clock;

use super::dataset::{EvaluationDatasetEffect, GenerateDatasetEffect};

const ATTEMPT_MULTIPLIER: usize = 12;
const PREVIOUS_QUESTION_PROMPT_LIMIT: usize = 12;

pub struct EvaluationDatasetEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    generator: Arc<dyn EvaluationGenerator>,
    embedding_service: Arc<EmbeddingService>,
    command_processor: Arc<CommandProcessor<EvaluationDataset>>,
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    clock: Arc<dyn Clock>,
}

impl EvaluationDatasetEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        generator: Arc<dyn EvaluationGenerator>,
        embedding_service: Arc<EmbeddingService>,
        command_processor: Arc<CommandProcessor<EvaluationDataset>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            blob_store,
            generator,
            embedding_service,
            command_processor,
            job_registry,
            activity_registry,
            clock,
        })
    }

    async fn run_generation(&self, effect: &GenerateDatasetEffect) -> Result<(), AppError> {
        let doc = self
            .source_document_repository
            .load(effect.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", effect.document_id)))?;

        let bytes = self.blob_store.get(&doc.latest_content_hash).await?;
        let plain_text = String::from_utf8(bytes)
            .map_err(|e| AppError::Internal(format!("document content is not valid UTF-8: {e}")))?;

        let embedding_model = self
            .embedding_service
            .resolve(effect.embedding_model_id)
            .await?;
        let target = effect.target_question_count as usize;
        let max_attempts = (target * ATTEMPT_MULTIPLIER).max(target + 12);
        let excerpt_threshold = effect.excerpt_similarity_threshold_milli as f32 / 1000.0;
        let duplicate_threshold = effect.duplicate_similarity_threshold_milli as f32 / 1000.0;

        let mut gate = GeneratedQuestionGate::new(
            self.embedding_service.as_ref(),
            &embedding_model,
            excerpt_threshold,
            duplicate_threshold,
        );
        let mut previous_coverage: Vec<String> = Vec::new();
        let mut rejection_attempt: u32 = 0;
        let mut accepted_sequence: u32 = 0;

        let (job_id, job) = self.job_registry.create().await;
        let stream_url = format!("/api/job/logs/{job_id}");

        self.activity_registry
            .attach_stream(effect.dataset_id, stream_url)
            .await;

        job.emit(
            InternalLogEvent::info(format!(
                "Starting dataset generation · target {target} questions ({} attempts max)",
                max_attempts,
            ))
            .with_meta("dataset_id", json!(effect.dataset_id.to_string()))
            .with_meta("document_id", json!(effect.document_id.to_string()))
            .with_meta("target", json!(target))
            .with_meta("max_attempts", json!(max_attempts))
            .with_meta(
                "generation_model_id",
                json!(effect.generation_model_id.to_string()),
            )
            .with_meta("excerpt_threshold", json!(excerpt_threshold))
            .with_meta("duplicate_threshold", json!(duplicate_threshold)),
        )
        .await;

        for attempt in 0..max_attempts {
            if gate.kept_count() >= target {
                break;
            }

            let prompt =
                build_question_prompt(&plain_text, recent_previous_coverage(&previous_coverage));
            let generated_result = self
                .generator
                .generate_question(effect.generation_model_id, prompt)
                .await;

            let generated = match generated_result {
                Ok(g) => g,
                Err(e) => {
                    job.emit(
                        InternalLogEvent::warn(format!(
                            "Generation attempt {} failed",
                            attempt + 1,
                        ))
                        .with_meta("attempt", json!(attempt + 1))
                        .with_meta("error", json!(e.to_string())),
                    )
                    .await;
                    continue;
                }
            };

            let shared_question =
                match ReferenceLocator::generated_to_question(&generated, &plain_text) {
                    Ok(q) => q,
                    Err(e) => {
                        job.emit(
                            InternalLogEvent::info(format!(
                                "Discarded generated question (reference locator) on attempt {}",
                                attempt + 1,
                            ))
                            .with_meta("attempt", json!(attempt + 1))
                            .with_meta("error", json!(e.to_string())),
                        )
                        .await;
                        continue;
                    }
                };

            let decision = gate.try_accept(shared_question).await?;
            match decision {
                QuestionFilterDecision::Accepted { .. } => {
                    let q = gate
                        .latest_question()
                        .expect("gate.try_accept returned Accepted");
                    let references: Vec<EvaluationReference> = q
                        .references
                        .iter()
                        .map(|r| EvaluationReference {
                            content: r.content.clone(),
                            char_start: r.char_start,
                            char_end: r.char_end,
                            embedding: r.embedding.as_ref().map(|e| plain_f32_vec(e)),
                        })
                        .collect();

                    self.command_processor
                        .handle(
                            effect.dataset_id,
                            EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                                dataset_id: effect.dataset_id,
                                sequence: accepted_sequence,
                                question: q.question.clone(),
                                references,
                                embedding: None,
                                occurred_at: self.clock.now(),
                            }),
                        )
                        .await?;

                    job.emit(
                        InternalLogEvent::info(format!(
                            "Accepted question {}/{}",
                            gate.kept_count(),
                            target,
                        ))
                        .with_meta("sequence", json!(accepted_sequence))
                        .with_meta("kept", json!(gate.kept_count()))
                        .with_meta("target", json!(target))
                        .with_meta("question_preview", json!(truncate_str(&q.question, 200))),
                    )
                    .await;
                    accepted_sequence += 1;
                    previous_coverage.push(question_coverage_entry(q));
                }
                QuestionFilterDecision::RejectedLowExcerptSimilarity { similarity } => {
                    rejection_attempt += 1;
                    job.emit(
                        InternalLogEvent::info(format!(
                            "Rejected: low excerpt similarity ({:.1}%)",
                            similarity * 100.0,
                        ))
                        .with_meta("reason", json!("low_excerpt_similarity"))
                        .with_meta("attempt", json!(rejection_attempt))
                        .with_meta("similarity", json!(similarity)),
                    )
                    .await;
                    self.command_processor
                        .handle(
                            effect.dataset_id,
                            EvaluationDatasetCommand::RejectQuestion(RejectQuestion {
                                dataset_id: effect.dataset_id,
                                attempt: rejection_attempt,
                                reason: format!(
                                    "low excerpt similarity {:.1}%",
                                    similarity * 100.0
                                ),
                                occurred_at: self.clock.now(),
                            }),
                        )
                        .await?;
                }
                QuestionFilterDecision::RejectedDuplicate { similarity } => {
                    rejection_attempt += 1;
                    job.emit(
                        InternalLogEvent::info(format!(
                            "Rejected: duplicate ({:.1}% similar to a previous question)",
                            similarity * 100.0,
                        ))
                        .with_meta("reason", json!("duplicate"))
                        .with_meta("attempt", json!(rejection_attempt))
                        .with_meta("similarity", json!(similarity)),
                    )
                    .await;
                    self.command_processor
                        .handle(
                            effect.dataset_id,
                            EvaluationDatasetCommand::RejectQuestion(RejectQuestion {
                                dataset_id: effect.dataset_id,
                                attempt: rejection_attempt,
                                reason: format!("duplicate similarity {:.1}%", similarity * 100.0),
                                occurred_at: self.clock.now(),
                            }),
                        )
                        .await?;
                }
            }
        }

        if gate.kept_count() == 0 {
            job.emit(
                InternalLogEvent::warn("Dataset generation produced no usable questions")
                    .with_meta("rejection_attempts", json!(rejection_attempt))
                    .with_meta("max_attempts", json!(max_attempts)),
            )
            .await;
            self.command_processor
                .handle(
                    effect.dataset_id,
                    EvaluationDatasetCommand::FailDatasetGeneration(FailDatasetGeneration {
                        dataset_id: effect.dataset_id,
                        reason: "generator did not produce any usable questions".into(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
            return Err(AppError::Upstream(
                "generator did not produce any usable evaluation questions".into(),
            ));
        }

        if gate.kept_count() < target {
            let reason = format!(
                "generator produced only {}/{} usable questions after {} attempts",
                gate.kept_count(),
                target,
                max_attempts
            );
            job.emit(
                InternalLogEvent::warn(reason.clone())
                    .with_meta("kept", json!(gate.kept_count()))
                    .with_meta("target", json!(target))
                    .with_meta("max_attempts", json!(max_attempts))
                    .with_meta("rejection_attempts", json!(rejection_attempt)),
            )
            .await;
            self.command_processor
                .handle(
                    effect.dataset_id,
                    EvaluationDatasetCommand::FailDatasetGeneration(FailDatasetGeneration {
                        dataset_id: effect.dataset_id,
                        reason: reason.clone(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;
            return Err(AppError::Upstream(reason));
        }

        self.command_processor
            .handle(
                effect.dataset_id,
                EvaluationDatasetCommand::CompleteDatasetGeneration(CompleteDatasetGeneration {
                    dataset_id: effect.dataset_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        job.emit(
            InternalLogEvent::success(format!(
                "Dataset generation complete · {} accepted, {} rejected",
                gate.kept_count(),
                rejection_attempt,
            ))
            .with_meta("accepted", json!(gate.kept_count()))
            .with_meta("rejected", json!(rejection_attempt))
            .with_meta("target", json!(target)),
        )
        .await;

        job.finish().await;

        Ok(())
    }
}

#[async_trait]
impl EffectExecutor<EvaluationDatasetEffect> for EvaluationDatasetEffectExecutor {
    async fn execute(&self, effect: &EvaluationDatasetEffect) -> Result<(), AppError> {
        match effect {
            EvaluationDatasetEffect::GenerateDataset(e) => self.run_generation(e).await,
        }
    }
}

fn recent_previous_coverage(previous_coverage: &[String]) -> &[String] {
    let start = previous_coverage
        .len()
        .saturating_sub(PREVIOUS_QUESTION_PROMPT_LIMIT);
    &previous_coverage[start..]
}

fn question_coverage_entry(question: &crate::shared::EvaluationQuestionDto) -> String {
    let refs = question
        .references
        .iter()
        .take(2)
        .map(|r| truncate_str(&r.content, 160))
        .collect::<Vec<_>>()
        .join(" || ");
    format!(
        "Q: {} | Covered: {}",
        truncate_str(&question.question, 120),
        refs
    )
}

fn truncate_str(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let out = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        format!("{out}...")
    } else {
        out
    }
}
