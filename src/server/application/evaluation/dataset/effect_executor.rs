use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::comprehension::DocumentMapCommandHandler;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::evaluation::dataset::question_filter::{
    GeneratedQuestionGate, QuestionFilterDecision,
};
use crate::server::application::evaluation::dataset::question_prompt::build_question_prompt;
use crate::server::application::evaluation::dataset::seed_planner::{
    pick_axes, pick_role, plan_seed, rebind_seed_to_map, QuestionPlanTarget, QuestionSeed,
    UsedSeedRegistry,
};
use crate::server::application::evaluation::ports::EvaluationGenerator;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{ActivityRegistry, AppError, Job, JobRegistry};
use crate::server::domain::comprehension::map::aggregate::MapStatus;
use crate::server::domain::comprehension::map::repository::DocumentMapRepository;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::evaluation::dataset::aggregate::{
    DatasetGenerationStatus, EvaluationDataset,
};
use crate::server::domain::evaluation::dataset::commands::{
    AcceptQuestion, CompleteDatasetGeneration, EvaluationDatasetCommand, FailDatasetGeneration,
    RejectQuestion, ResolveMapDependency,
};
use crate::server::domain::evaluation::dataset::effects::{
    EnsureMapBuildEffect, EvaluationDatasetEffect, GenerateQuestionEffect, NotifyMapReadyEffect,
    PollMapStatusEffect,
};
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::question::{
    EvaluationQuestion, EvaluationReference, QuestionDimensions,
};
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::shared::contracts::EvaluationQuestionDto;
use crate::shared::plain_f32_vec;
use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::process_manager::{EffectError, EffectExecutor};

use crate::server::application::ports::Clock;

pub struct EvaluationDatasetEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    map_repository: Arc<dyn DocumentMapRepository>,
    map_command_handler: Arc<DocumentMapCommandHandler>,
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
        map_repository: Arc<dyn DocumentMapRepository>,
        map_command_handler: Arc<DocumentMapCommandHandler>,
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
            map_repository,
            map_command_handler,
            generator,
            embedding_service,
            command_processor,
            aggregate_repository,
            job_registry,
            activity_registry,
            clock,
        })
    }

    async fn execute_ensure_map(&self, effect: &EnsureMapBuildEffect) -> Result<(), AppError> {
        let Some(state) = self.load_active(effect.dataset_id).await? else {
            return Ok(());
        };
        let job = self.open_job(state.dataset_id).await;
        job.info(&format!(
            "Requesting comprehension map for document {}",
            state.document_id
        ))
        .await;

        match self
            .map_command_handler
            .request_build_for_document(
                state.document_id,
                state.document_version,
                state.generation_model_id,
            )
            .await
        {
            Ok(handle) => {
                if matches!(handle.status, MapStatus::Ready) {
                    self.resolve_map(state.dataset_id, handle.map_id, true, None)
                        .await?;
                } else {
                    self.poll_map(state.dataset_id).await?;
                }
                Ok(())
            }
            Err(e) => {
                job.error(&format!("Map build request failed: {e}")).await;
                self.resolve_map(state.dataset_id, state.map_id, false, Some(e.to_string()))
                    .await
            }
        }
    }

    async fn execute_poll_map(&self, effect: &PollMapStatusEffect) -> Result<(), AppError> {
        let Some(state) = self.load_active(effect.dataset_id).await? else {
            return Ok(());
        };
        self.poll_map(state.dataset_id).await
    }

    async fn execute_notify_map_ready(
        &self,
        effect: &NotifyMapReadyEffect,
    ) -> Result<(), AppError> {
        let awaiting = self
            .dataset_repository
            .find_awaiting_for_map(effect.map_id)
            .await
            .map_err(|e| AppError::Internal(format!("find_awaiting_for_map: {e}")))?;
        for dataset_id in awaiting {
            self.resolve_map(dataset_id, effect.map_id, true, None)
                .await?;
        }
        Ok(())
    }

    async fn poll_map(&self, dataset_id: Uuid) -> Result<(), AppError> {
        let Some(state) = self.load_active(dataset_id).await? else {
            return Ok(());
        };
        let Some(map) = self
            .map_repository
            .find_for_document(state.document_id, state.document_version)
            .await?
        else {
            return Ok(());
        };
        if map.status == "ready" {
            self.resolve_map(dataset_id, map.map_id, true, None).await?;
        }
        Ok(())
    }

    async fn resolve_map(
        &self,
        dataset_id: Uuid,
        map_id: Uuid,
        ready: bool,
        failure_reason: Option<String>,
    ) -> Result<(), AppError> {
        self.command_processor
            .handle(
                dataset_id,
                EvaluationDatasetCommand::ResolveMapDependency(ResolveMapDependency {
                    dataset_id,
                    map_id,
                    ready,
                    failure_reason,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }

    async fn execute_attempt(&self, effect: &GenerateQuestionEffect) -> Result<(), AppError> {
        let Some(state) = self.load_active(effect.dataset_id).await? else {
            return Ok(());
        };
        let job = self.open_job(state.dataset_id).await;

        if !matches!(state.status, DatasetGenerationStatus::Generating) {
            self.poll_map(state.dataset_id).await?;
            return Ok(());
        }

        let Some(detail) = self.map_repository.load_detail(state.map_id).await? else {
            job.warn("Map not ready yet; polling again").await;
            self.poll_map(state.dataset_id).await?;
            return Ok(());
        };

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

        let rng_seed = (state.attempt_count as u64).wrapping_add(state.dataset_id.as_u128() as u64);
        let Some((operation, evidence)) = pick_axes(
            &state.weight_matrix,
            &state.accepted_by_axes,
            state.target_question_count,
            rng_seed,
        ) else {
            job.warn("No valid axis combination in weight matrix").await;
            self.emit_reject(state.dataset_id, "no valid axis to seed from".into())
                .await?;
            return self.maybe_terminate(state.dataset_id, &job).await;
        };
        let role: SuggestedRole = pick_role(&detail.read_model.suggested_roles, rng_seed);

        let clean_accepted = state.clean_accepted_count();
        let target = state.target_question_count;
        let attempt_number = state.attempt_count + 1;
        job.info(&format!(
            "Attempt {attempt_number} · {}/{} · ({}, {})",
            clean_accepted,
            target,
            operation.label(),
            evidence.label(),
        ))
        .await;

        let used = build_used_registry(&existing_questions);
        let seed: QuestionSeed = match plan_seed(
            &QuestionPlanTarget {
                operation,
                evidence,
                role: role.clone(),
            },
            &detail,
            &used,
            rng_seed,
        ) {
            Ok(s) => s,
            Err(e) => {
                job.info(&format!("Rejected: seed planner: {e}")).await;
                self.emit_reject(state.dataset_id, format!("seed planner: {e}"))
                    .await?;
                return self.maybe_terminate(state.dataset_id, &job).await;
            }
        };

        let dimensions = QuestionDimensions {
            operation,
            evidence,
            role: role.name.clone(),
        };
        let prompt = build_question_prompt(&seed, &role);

        let generated = match self
            .generator
            .generate_question(state.generation_model_id, prompt, dimensions.clone())
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

        let mut gate = GeneratedQuestionGate::with_existing(
            self.embedding_service.as_ref(),
            &embedding_model,
            state.duplicate_similarity_threshold_milli as f32 / 1000.0,
            existing_dtos,
        );
        let decision = gate.try_accept(&generated.question).await?;
        match decision {
            QuestionFilterDecision::Accepted { embedding } => {
                let references = build_references(&plain_text, state.document_id, &seed);
                let next_sequence = state.next_sequence();
                let display_number = next_sequence + 1;
                let question_preview = truncate_str(&generated.question, 160);
                let evidence_refs = rebind_seed_to_map(&seed, state.map_id);
                self.command_processor
                    .handle(
                        state.dataset_id,
                        EvaluationDatasetCommand::AcceptQuestion(AcceptQuestion {
                            dataset_id: state.dataset_id,
                            sequence: next_sequence,
                            question: generated.question.clone(),
                            references,
                            embedding: Some(embedding),
                            dimensions: dimensions.clone(),
                            evidence_refs,
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
        let _ = plain_f32_vec; // ensure import stays even when unused

        self.maybe_terminate(state.dataset_id, &job).await
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
            Some(l)
                if matches!(
                    l.aggregate.status,
                    DatasetGenerationStatus::Completed
                        | DatasetGenerationStatus::Cancelled
                        | DatasetGenerationStatus::Failed { .. }
                ) =>
            {
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
        let Some(loaded) = self.aggregate_repository.load(dataset_id).await? else {
            return Ok(());
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
                let reason = format!("no usable questions after {attempts} attempts");
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

fn build_used_registry(existing: &[EvaluationQuestion]) -> UsedSeedRegistry<'static> {
    use crate::server::domain::comprehension::map_item::MapItemRef;
    let mut reg = UsedSeedRegistry::empty();
    for q in existing {
        for r in &q.evidence_refs {
            match r {
                MapItemRef::Observation { observation_id, .. } => {
                    reg.seen_observation_ids.push(*observation_id);
                }
                MapItemRef::Thread { thread_id, .. } => reg.seen_thread_ids.push(*thread_id),
                MapItemRef::Insight { insight_id, .. } => reg.seen_insight_ids.push(*insight_id),
                MapItemRef::Connection { .. } => {}
            }
        }
    }
    reg
}

fn build_references(
    plain_text: &str,
    document_id: Uuid,
    seed: &QuestionSeed,
) -> Vec<EvaluationReference> {
    seed.spans
        .iter()
        .map(|s| EvaluationReference {
            document_id,
            content: slice_text(plain_text, s.char_start, s.char_end),
            char_start: s.char_start,
            char_end: s.char_end,
            embedding: None,
        })
        .collect()
}

fn slice_text(text: &str, start: u32, end: u32) -> String {
    let start = start as usize;
    let end = end as usize;
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
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

#[async_trait]
impl EffectExecutor<EvaluationDatasetEffect> for EvaluationDatasetEffectExecutor {
    async fn execute(&self, effect: &EvaluationDatasetEffect) -> Result<(), EffectError> {
        let result: Result<(), AppError> = match effect {
            EvaluationDatasetEffect::EnsureMapBuild(e) => self.execute_ensure_map(e).await,
            EvaluationDatasetEffect::PollMapStatus(e) => self.execute_poll_map(e).await,
            EvaluationDatasetEffect::AttemptQuestionGeneration(e) => self.execute_attempt(e).await,
            EvaluationDatasetEffect::NotifyMapReady(e) => self.execute_notify_map_ready(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}
