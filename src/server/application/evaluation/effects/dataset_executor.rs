use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::server::application::embedding::EmbeddingService;
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

    #[allow(clippy::too_many_arguments)]
    async fn try_emit_paraphrase(
        &self,
        effect: &GenerateDatasetEffect,
        job: &Arc<crate::server::application::Job>,
        clean_sequence: u32,
        clean_question: &str,
        clean_embedding: Option<&[f32]>,
        clean_references: &[EvaluationReference],
        category: crate::server::domain::evaluation::question::QuestionCategory,
        embedding_model: &crate::server::application::embedding::ResolvedEmbeddingModel,
        new_sequence: u32,
    ) -> Result<bool, AppError> {
        let prompt = crate::server::application::evaluation::generator::build_paraphrase_prompt(
            clean_question,
        );
        let paraphrase = self
            .generator
            .paraphrase_question(effect.generation_model_id, prompt)
            .await?;

        let mut paraphrase_embeddings = self
            .embedding_service
            .embed_with_resolved(embedding_model, std::slice::from_ref(&paraphrase))
            .await?;
        let Some(paraphrase_embedding) = paraphrase_embeddings.pop() else {
            return Ok(false);
        };

        if let Some(original_embedding) = clean_embedding {
            let cosine = cosine_similarity(original_embedding, &paraphrase_embedding);
            if cosine < crate::server::application::evaluation::generator::PARAPHRASE_MIN_COSINE {
                job.emit(
                    InternalLogEvent::info(format!(
                        "Paraphrase rejected (cosine {:.2} < min)",
                        cosine,
                    ))
                    .with_meta("clean_sequence", json!(clean_sequence))
                    .with_meta("cosine", json!(cosine)),
                )
                .await;
                return Ok(false);
            }
        }

        self.command_processor
            .handle(
                effect.dataset_id,
                EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                    dataset_id: effect.dataset_id,
                    sequence: new_sequence,
                    question: paraphrase.clone(),
                    references: clean_references.to_vec(),
                    embedding: Some(paraphrase_embedding.clone()),
                    category,
                    grammar_variant:
                        crate::server::domain::evaluation::question::GrammarVariant::Broken,
                    paraphrase_of: Some(clean_sequence),
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;

        job.emit(
            InternalLogEvent::info(format!("Accepted broken paraphrase of Q{}", clean_sequence,))
                .with_meta("clean_sequence", json!(clean_sequence))
                .with_meta("paraphrase_preview", json!(truncate_str(&paraphrase, 200))),
        )
        .await;

        Ok(true)
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

        let plan =
            crate::server::application::evaluation::generator::GenerationPlan::default_for_count(
                effect.target_question_count,
            );
        let mut remaining_by_category: std::collections::HashMap<
            crate::server::domain::evaluation::question::QuestionCategory,
            u32,
        > = plan.by_category.clone();
        let mut accepted_by_category: std::collections::HashMap<
            crate::server::domain::evaluation::question::QuestionCategory,
            u32,
        > = std::collections::HashMap::new();

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

            let category = remaining_by_category
                .iter()
                .filter(|(_, n)| **n > 0)
                .max_by_key(|(_, n)| **n)
                .map(|(c, _)| *c)
                .unwrap_or(
                    crate::server::domain::evaluation::question::QuestionCategory::FactRetrieval,
                );
            let prompt =
                crate::server::application::evaluation::generator::build_question_prompt_for(
                    &plain_text,
                    recent_previous_coverage(&previous_coverage),
                    category,
                );
            let generated_result = self
                .generator
                .generate_question(effect.generation_model_id, prompt, category)
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

                    let clean_sequence = accepted_sequence;
                    let clean_question_text = q.question.clone();
                    let clean_references = references.clone();
                    let clean_embedding = q.embedding.as_ref().map(|e| plain_f32_vec(e));

                    self.command_processor
                        .handle(
                            effect.dataset_id,
                            EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                                dataset_id: effect.dataset_id,
                                sequence: clean_sequence,
                                question: clean_question_text.clone(),
                                references,
                                embedding: None,
                                category,
                                grammar_variant:
                                    crate::server::domain::evaluation::question::GrammarVariant::Clean,
                                paraphrase_of: None,
                                occurred_at: self.clock.now(),
                            }),
                        )
                        .await?;
                    let r = remaining_by_category.entry(category).or_insert(0);
                    *r = r.saturating_sub(1);
                    *accepted_by_category.entry(category).or_insert(0) += 1;

                    job.emit(
                        InternalLogEvent::info(format!(
                            "Accepted question {}/{}",
                            gate.kept_count(),
                            target,
                        ))
                        .with_meta("sequence", json!(clean_sequence))
                        .with_meta("kept", json!(gate.kept_count()))
                        .with_meta("target", json!(target))
                        .with_meta("question_preview", json!(truncate_str(&q.question, 200))),
                    )
                    .await;
                    accepted_sequence += 1;
                    previous_coverage.push(question_coverage_entry(q));

                    if effect.grammar_variants_enabled {
                        match self
                            .try_emit_paraphrase(
                                effect,
                                &job,
                                clean_sequence,
                                &clean_question_text,
                                clean_embedding.as_deref(),
                                &clean_references,
                                category,
                                &embedding_model,
                                accepted_sequence,
                            )
                            .await
                        {
                            Ok(true) => accepted_sequence += 1,
                            Ok(false) => {}
                            Err(e) => {
                                job.emit(
                                    InternalLogEvent::warn(format!(
                                        "Paraphrase pass failed for Q{}: {e}",
                                        clean_sequence
                                    ))
                                    .with_meta("clean_sequence", json!(clean_sequence)),
                                )
                                .await;
                            }
                        }
                    }
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom <= 1e-9 {
        0.0
    } else {
        dot / denom
    }
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
