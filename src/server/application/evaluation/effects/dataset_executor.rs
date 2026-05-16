use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::contracts::EvaluationQuestionDto;
use crate::core::plain_f32_vec;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::generator::{
    build_paraphrase_prompt, build_question_prompt_for, GenerationPlan, PARAPHRASE_MIN_COSINE,
};
use crate::server::application::evaluation::ports::EvaluationGenerator;
use crate::server::application::evaluation::question_filter::{
    GeneratedQuestionGate, QuestionFilterDecision,
};
use crate::server::application::evaluation::reference_locator::ReferenceLocator;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{ActivityRegistry, AppError, Job, JobRegistry};
use crate::server::domain::evaluation::dataset::aggregate::{
    DatasetGenerationStatus, EvaluationDataset,
};
use crate::server::domain::evaluation::dataset::commands::{
    AcceptQuestion, CompleteDatasetGeneration, EvaluationDatasetCommand, FailDatasetGeneration,
    RejectQuestion,
};
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::question::{
    EvaluationQuestion, EvaluationReference, GrammarVariant, QuestionCategory,
};
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::event_sourcing::aggregate_repository::AggregateRepository;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::process_manager::EffectExecutor;

use crate::server::application::ports::Clock;

use super::dataset::{EvaluationDatasetEffect, GenerateParaphraseEffect, GenerateQuestionEffect};

const PREVIOUS_QUESTION_PROMPT_LIMIT: usize = 12;

pub struct EvaluationDatasetEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    generator: Arc<dyn EvaluationGenerator>,
    embedding_service: Arc<EmbeddingService>,
    command_processor: Arc<CommandProcessor<EvaluationDataset>>,
    aggregate_repository: Arc<AggregateRepository<EvaluationDataset>>,
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    clock: Arc<dyn Clock>,
}

impl EvaluationDatasetEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        dataset_repository: Arc<dyn EvaluationDatasetRepository>,
        generator: Arc<dyn EvaluationGenerator>,
        embedding_service: Arc<EmbeddingService>,
        command_processor: Arc<CommandProcessor<EvaluationDataset>>,
        aggregate_repository: Arc<AggregateRepository<EvaluationDataset>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            blob_store,
            dataset_repository,
            generator,
            embedding_service,
            command_processor,
            aggregate_repository,
            job_registry,
            activity_registry,
            clock,
        })
    }

    async fn execute_attempt(&self, effect: &GenerateQuestionEffect) -> Result<(), AppError> {
        let state = match self.load_active(effect.dataset_id).await? {
            Some(s) => s,
            None => return Ok(()),
        };
        let job = self.open_job(state.dataset_id).await;

        let document = self
            .source_document_repository
            .load(state.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", state.document_id)))?;
        let bytes = self.blob_store.get(&document.latest_content_hash).await?;
        let plain_text = String::from_utf8(bytes)
            .map_err(|e| AppError::Internal(format!("document is not valid UTF-8: {e}")))?;

        let embedding_model = self
            .embedding_service
            .resolve(state.embedding_model_id)
            .await?;

        let existing_questions = self
            .dataset_repository
            .load_questions(state.dataset_id)
            .await
            .map_err(|e| AppError::Internal(format!("load questions: {e}")))?;
        let existing_dtos: Vec<EvaluationQuestionDto> =
            existing_questions.iter().cloned().map(Into::into).collect();
        let recent_coverage = recent_coverage(&existing_questions);

        let category = pick_category(&state);
        let clean_accepted = state.clean_accepted_count();
        let target = state.target_question_count;
        let attempt_number = state.attempt_count + 1;

        job.info(&format!(
            "Attempt {attempt_number} · {} · {clean_accepted}/{target} accepted",
            category.label(),
        ))
        .await;

        let mut gate = GeneratedQuestionGate::with_existing(
            self.embedding_service.as_ref(),
            &embedding_model,
            state.excerpt_similarity_threshold_milli as f32 / 1000.0,
            state.duplicate_similarity_threshold_milli as f32 / 1000.0,
            existing_dtos,
        );

        let prompt = build_question_prompt_for(&plain_text, &recent_coverage, category);
        let generated = match self
            .generator
            .generate_question(state.generation_model_id, prompt, category)
            .await
        {
            Ok(g) => g,
            Err(e) => {
                job.warn(&format!("Generator call failed: {e}")).await;
                self.emit_reject(state.dataset_id, format!("generator error: {e}"))
                    .await?;
                return self.maybe_terminate(state.dataset_id, &job).await;
            }
        };

        let shared = match ReferenceLocator::generated_to_question(&generated, &plain_text) {
            Ok(q) => q,
            Err(e) => {
                job.info(&format!("Rejected: reference locator: {e}")).await;
                self.emit_reject(state.dataset_id, format!("reference locator: {e}"))
                    .await?;
                return self.maybe_terminate(state.dataset_id, &job).await;
            }
        };

        let decision = gate.try_accept(shared).await?;
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
                let embedding = q.embedding.as_ref().map(|e| plain_f32_vec(e));

                let next_sequence = state.next_sequence();
                let display_number = next_sequence + 1;
                let question_preview = truncate_str(&q.question, 160);
                self.command_processor
                    .handle(
                        state.dataset_id,
                        EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                            dataset_id: state.dataset_id,
                            sequence: next_sequence,
                            question: q.question.clone(),
                            references,
                            embedding,
                            category,
                            grammar_variant: GrammarVariant::Clean,
                            paraphrase_of: None,
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                job.info(&format!(
                    "Q{display_number} ({}/{}): {question_preview}",
                    clean_accepted + 1,
                    target,
                ))
                .await;
            }
            QuestionFilterDecision::RejectedLowExcerptSimilarity { similarity } => {
                job.info(&format!(
                    "Rejected: weak source match ({:.0}%)",
                    similarity * 100.0,
                ))
                .await;
                self.emit_reject(
                    state.dataset_id,
                    format!("low excerpt similarity {:.1}%", similarity * 100.0),
                )
                .await?;
            }
            QuestionFilterDecision::RejectedDuplicate { similarity } => {
                job.info(&format!(
                    "Rejected: duplicate ({:.0}% similar)",
                    similarity * 100.0,
                ))
                .await;
                self.emit_reject(
                    state.dataset_id,
                    format!("duplicate similarity {:.1}%", similarity * 100.0),
                )
                .await?;
            }
        }

        self.maybe_terminate(state.dataset_id, &job).await
    }

    async fn execute_paraphrase(&self, effect: &GenerateParaphraseEffect) -> Result<(), AppError> {
        let state = match self.load_active(effect.dataset_id).await? {
            Some(s) => s,
            None => return Ok(()),
        };
        let job = self.open_job(state.dataset_id).await;

        if !state.grammar_variants_enabled {
            return Ok(());
        }

        let existing_questions = self
            .dataset_repository
            .load_questions(state.dataset_id)
            .await
            .map_err(|e| AppError::Internal(format!("load questions: {e}")))?;
        let clean_display = effect.clean_sequence + 1;
        let clean = match existing_questions
            .iter()
            .find(|q| q.sequence == effect.clean_sequence)
        {
            Some(q) => q,
            None => {
                job.warn(&format!(
                    "Paraphrase skipped: Q{clean_display} not found"
                ))
                .await;
                return Ok(());
            }
        };

        let embedding_model = self
            .embedding_service
            .resolve(state.embedding_model_id)
            .await?;
        let prompt = build_paraphrase_prompt(&clean.question);
        let paraphrase = match self
            .generator
            .paraphrase_question(state.generation_model_id, prompt)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                job.warn(&format!("Paraphrase failed for Q{clean_display}: {e}"))
                    .await;
                return Ok(());
            }
        };

        let mut embeddings = self
            .embedding_service
            .embed_with_resolved(&embedding_model, std::slice::from_ref(&paraphrase))
            .await?;
        let Some(paraphrase_embedding) = embeddings.pop() else {
            return Ok(());
        };

        if let Some(original) = clean.embedding.as_ref() {
            let cosine = cosine_similarity(original, &paraphrase_embedding);
            if cosine < PARAPHRASE_MIN_COSINE {
                job.info(&format!(
                    "Paraphrase of Q{clean_display} rejected (drifted too far, {:.0}% match)",
                    cosine * 100.0,
                ))
                .await;
                return Ok(());
            }
        }

        let references: Vec<EvaluationReference> = clean
            .references
            .iter()
            .map(|r| EvaluationReference {
                content: r.content.clone(),
                char_start: r.char_start,
                char_end: r.char_end,
                embedding: r.embedding.clone(),
            })
            .collect();

        let next_sequence = state.next_sequence();
        let display_number = next_sequence + 1;
        let preview = truncate_str(&paraphrase, 160);
        self.command_processor
            .handle(
                state.dataset_id,
                EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                    dataset_id: state.dataset_id,
                    sequence: next_sequence,
                    question: paraphrase,
                    references,
                    embedding: Some(paraphrase_embedding),
                    category: effect.category,
                    grammar_variant: GrammarVariant::Broken,
                    paraphrase_of: Some(effect.clean_sequence),
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        job.info(&format!(
            "Q{display_number} (paraphrase of Q{clean_display}): {preview}"
        ))
        .await;

        Ok(())
    }

    async fn open_job(&self, dataset_id: Uuid) -> Arc<Job> {
        let job_id = dataset_id.to_string();
        let (job, created) = self.job_registry.get_or_create(job_id.clone()).await;
        if created {
            let stream_url = format!("/api/job/logs/{job_id}");
            self.activity_registry
                .attach_stream(dataset_id, stream_url)
                .await;
        }
        job
    }

    async fn load_active(&self, dataset_id: Uuid) -> Result<Option<EvaluationDataset>, AppError> {
        let loaded = self.aggregate_repository.load(dataset_id).await?;
        match loaded {
            None => Ok(None),
            Some(l) if !matches!(l.aggregate.status, DatasetGenerationStatus::Generating) => {
                Ok(None)
            }
            Some(l) => Ok(Some(l.aggregate)),
        }
    }

    async fn emit_reject(&self, dataset_id: Uuid, reason: String) -> Result<(), AppError> {
        let loaded = self.aggregate_repository.load(dataset_id).await?;
        let attempt = loaded
            .map(|l| l.aggregate.attempt_count.saturating_add(1))
            .unwrap_or(1);
        self.command_processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::RejectQuestion(RejectQuestion {
                    dataset_id,
                    attempt,
                    reason,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }

    async fn maybe_terminate(&self, dataset_id: Uuid, job: &Arc<Job>) -> Result<(), AppError> {
        let loaded = match self.aggregate_repository.load(dataset_id).await? {
            Some(l) => l,
            None => return Ok(()),
        };
        let state = loaded.aggregate;
        if !matches!(state.status, DatasetGenerationStatus::Generating) {
            return Ok(());
        }

        let clean = state.clean_accepted_count();
        let target = state.target_question_count;
        let attempts = state.attempt_count;
        let cap = state.max_attempts;

        if clean >= target {
            self.command_processor
                .handle(
                    dataset_id,
                    EvaluationDatasetCommand::CompleteDatasetGeneration(
                        CompleteDatasetGeneration {
                            dataset_id,
                            occurred_at: self.clock.now(),
                        },
                    ),
                )
                .await?;
            job.info(&format!(
                "Done · {clean}/{target} questions in {attempts} attempts"
            ))
            .await;
            job.finish().await;
        } else if attempts >= cap {
            if clean > 0 {
                self.command_processor
                    .handle(
                        dataset_id,
                        EvaluationDatasetCommand::CompleteDatasetGeneration(
                            CompleteDatasetGeneration {
                                dataset_id,
                                occurred_at: self.clock.now(),
                            },
                        ),
                    )
                    .await?;
                job.warn(&format!(
                    "Stopping early · hit attempt cap after {attempts}, kept {clean}/{target}"
                ))
                .await;
                job.finish().await;
            } else {
                let reason =
                    format!("no usable questions after {attempts} attempts");
                self.command_processor
                    .handle(
                        dataset_id,
                        EvaluationDatasetCommand::FailDatasetGeneration(FailDatasetGeneration {
                            dataset_id,
                            reason: reason.clone(),
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                job.error(&format!("Generation failed · {reason}")).await;
                job.finish().await;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl EffectExecutor<EvaluationDatasetEffect> for EvaluationDatasetEffectExecutor {
    async fn execute(&self, effect: &EvaluationDatasetEffect) -> Result<(), AppError> {
        match effect {
            EvaluationDatasetEffect::AttemptQuestionGeneration(e) => self.execute_attempt(e).await,
            EvaluationDatasetEffect::GenerateParaphrase(e) => self.execute_paraphrase(e).await,
        }
    }
}

fn pick_category(state: &EvaluationDataset) -> QuestionCategory {
    let plan = GenerationPlan::default_for_count(state.target_question_count);
    let mut best: Option<(QuestionCategory, i64)> = None;
    for category in QuestionCategory::all() {
        let target = *plan.by_category.get(&category).unwrap_or(&0) as i64;
        let accepted = *state.accepted_by_category.get(&category).unwrap_or(&0) as i64;
        let remaining = target - accepted;
        match best {
            None => best = Some((category, remaining)),
            Some((_, current)) if remaining > current => best = Some((category, remaining)),
            _ => {}
        }
    }
    best.map(|(c, _)| c)
        .unwrap_or(QuestionCategory::FactRetrieval)
}

fn recent_coverage(questions: &[EvaluationQuestion]) -> Vec<String> {
    let clean: Vec<&EvaluationQuestion> = questions
        .iter()
        .filter(|q| q.paraphrase_of.is_none())
        .collect();
    let start = clean.len().saturating_sub(PREVIOUS_QUESTION_PROMPT_LIMIT);
    clean[start..].iter().map(|q| coverage_entry(q)).collect()
}

fn coverage_entry(q: &EvaluationQuestion) -> String {
    let refs = q
        .references
        .iter()
        .take(2)
        .map(|r| truncate_str(&r.content, 160))
        .collect::<Vec<_>>()
        .join(" || ");
    format!("Q: {} | Covered: {}", truncate_str(&q.question, 120), refs)
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
