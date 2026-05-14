use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::PipelineResolver;
use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::evaluation::ports::{RetrievalQuery, Retriever};
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{ActivityRegistry, AppError, InternalLogEvent, Job, JobRegistry};
use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::embedding_set::entity::{ChunkEmbedding, EmbeddingSet};
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::question::EvaluationQuestion;
use crate::server::domain::evaluation::run::aggregate::EvaluationRun;
use crate::server::domain::evaluation::run::commands::{
    CompleteRun, EvaluationRunCommand, FailRun, MarkVariantPrepared, ScoreVariant,
};
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::scoring::{mean, precision_omega, score_question, std_dev};
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::process_manager::EffectExecutor;
use crate::shared::{
    ChunkingVariant, EvaluationMetrics, EvaluationResultSplit, EvaluationRunOptions,
};

use super::run::{EvaluationRunEffect, ExecuteRunEffect};

pub struct EvaluationRunEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    chunker_registry: Arc<ChunkerRegistry>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    embedding_service: Arc<EmbeddingService>,
    embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
    dataset_repository: Arc<dyn EvaluationDatasetRepository>,
    retriever: Arc<dyn Retriever>,
    command_processor: Arc<CommandProcessor<EvaluationRun>>,
    pipeline_resolver: Arc<PipelineResolver>,
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl EvaluationRunEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        blob_store: Arc<dyn BlobStore>,
        chunker_registry: Arc<ChunkerRegistry>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        embedding_service: Arc<EmbeddingService>,
        embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
        dataset_repository: Arc<dyn EvaluationDatasetRepository>,
        retriever: Arc<dyn Retriever>,
        command_processor: Arc<CommandProcessor<EvaluationRun>>,
        pipeline_resolver: Arc<PipelineResolver>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            blob_store,
            chunker_registry,
            chunk_set_repository,
            embedding_service,
            embedding_set_repository,
            dataset_repository,
            retriever,
            command_processor,
            pipeline_resolver,
            job_registry,
            activity_registry,
            clock,
            id_generator,
        })
    }

    async fn run(&self, effect: &ExecuteRunEffect) -> Result<(), AppError> {
        let (job_id, job) = self.job_registry.create().await;
        let stream_url = format!("/api/job/logs/{job_id}");
        self.activity_registry
            .attach_stream(effect.run_id, stream_url)
            .await;

        let result = self.run_inner(effect, job.clone()).await;
        if let Err(e) = &result {
            job.error(&format!("Evaluation run failed: {e}")).await;
            let _ = self
                .command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::FailRun(FailRun {
                        run_id: effect.run_id,
                        reason: e.to_string(),
                        occurred_at: self.clock.now(),
                    }),
                )
                .await;
        }
        job.finish().await;
        result
    }

    async fn run_inner(&self, effect: &ExecuteRunEffect, job: Arc<Job>) -> Result<(), AppError> {
        if effect.autotune_request.is_some() {
            return Err(AppError::Validation(
                "autotune is not yet implemented in the new evaluation path".into(),
            ));
        }

        let dataset = self
            .dataset_repository
            .load(effect.dataset_id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| {
                AppError::NotFound(format!("evaluation dataset {}", effect.dataset_id))
            })?;

        let questions = self
            .dataset_repository
            .load_questions(effect.dataset_id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if questions.is_empty() {
            return Err(AppError::Validation(
                "evaluation dataset has no questions".into(),
            ));
        }

        let doc = self
            .source_document_repository
            .load(dataset.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", dataset.document_id)))?;

        let bytes = self.blob_store.get(&doc.latest_content_hash).await?;
        let plain_text = String::from_utf8(bytes)
            .map_err(|e| AppError::Internal(format!("document content is not valid UTF-8: {e}")))?;

        let pipeline = self
            .pipeline_resolver
            .resolve(effect.pipeline_configuration_id)
            .await?;
        let embedding_model = &pipeline.embedding_model;

        let question_texts: Vec<String> = questions.iter().map(|q| q.question.clone()).collect();

        job.emit(
            InternalLogEvent::info(format!(
                "Starting evaluation run: {} variants × {} options across {} questions ({})",
                effect.variants.len(),
                effect.options.len(),
                question_texts.len(),
                embedding_model.model,
            ))
            .with_meta("run_id", json!(effect.run_id.to_string()))
            .with_meta("dataset_id", json!(effect.dataset_id.to_string()))
            .with_meta("question_count", json!(question_texts.len()))
            .with_meta("variant_count", json!(effect.variants.len()))
            .with_meta("option_count", json!(effect.options.len()))
            .with_meta("embedding_model", json!(embedding_model.model)),
        )
        .await;

        let question_embeddings = self
            .embedding_service
            .embed_with_resolved(embedding_model, &question_texts)
            .await?;

        for variant in &effect.variants {
            job.emit(
                InternalLogEvent::info(format!("Preparing variant '{}'…", variant.label))
                    .with_meta("variant_label", json!(variant.label)),
            )
            .await;

            let (chunk_set_id, chunks) = self
                .find_or_create_chunk_set(
                    dataset.document_id,
                    dataset.document_version,
                    &plain_text,
                    variant,
                )
                .await?;

            let embedding_set_id = self
                .find_or_create_embedding_set(chunk_set_id, &chunks, embedding_model)
                .await?;

            self.command_processor
                .handle(
                    effect.run_id,
                    EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                        run_id: effect.run_id,
                        variant_label: variant.label.clone(),
                        chunk_set_id,
                        embedding_set_id,
                        occurred_at: self.clock.now(),
                    }),
                )
                .await?;

            job.emit(
                InternalLogEvent::info(format!(
                    "Variant '{}' prepared: {} chunks",
                    variant.label,
                    chunks.len(),
                ))
                .with_meta("variant_label", json!(variant.label))
                .with_meta("chunk_count", json!(chunks.len()))
                .with_meta("chunk_set_id", json!(chunk_set_id.to_string()))
                .with_meta("embedding_set_id", json!(embedding_set_id.to_string())),
            )
            .await;

            let splits = vec![EvaluationResultSplit::Full];

            for options in &effect.options {
                for split in &splits {
                    let (metrics, traces) = self
                        .score_variant(
                            embedding_set_id,
                            &questions,
                            &chunks,
                            &question_embeddings,
                            options,
                        )
                        .await?;

                    job.emit(
                        InternalLogEvent::info(format!(
                            "Scored variant '{}' (top_k={}, split={}): recall={:.3} precision={:.3} iou={:.3}",
                            variant.label,
                            options.top_k,
                            split.as_str(),
                            metrics.recall_mean,
                            metrics.precision_mean,
                            metrics.iou_mean,
                        ))
                        .with_meta("variant_label", json!(variant.label))
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

                    self.command_processor
                        .handle(
                            effect.run_id,
                            EvaluationRunCommand::ScoreVariant(ScoreVariant {
                                run_id: effect.run_id,
                                variant_label: variant.label.clone(),
                                variant_config: variant.config,
                                options: options.clone(),
                                split: *split,
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

        job.emit(
            InternalLogEvent::success(format!(
                "Evaluation run complete · {} variants × {} options scored",
                effect.variants.len(),
                effect.options.len(),
            ))
            .with_meta("run_id", json!(effect.run_id.to_string()))
            .with_meta("variant_count", json!(effect.variants.len()))
            .with_meta("option_count", json!(effect.options.len())),
        )
        .await;

        Ok(())
    }

    async fn find_or_create_chunk_set(
        &self,
        document_id: Uuid,
        document_version: u32,
        plain_text: &str,
        variant: &ChunkingVariant,
    ) -> Result<(Uuid, Vec<Chunk>), AppError> {
        let existing = self
            .chunk_set_repository
            .list_for_document(document_id)
            .await?;

        if let Some(cs) = existing.iter().find(|cs| {
            cs.document_version == document_version && cs.chunking_config == variant.config
        }) {
            let chunks = self
                .chunk_set_repository
                .load_chunks(cs.chunk_set_id)
                .await?;
            return Ok((cs.chunk_set_id, chunks));
        }

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(&variant.config, plain_text)
            .await
            .map_err(|e| AppError::Internal(format!("chunking failed: {e}")))?;

        let chunk_set_id = self.id_generator.new_uuid();
        let occurred_at = self.clock.now();
        let chunks: Vec<Chunk> = chunk_outputs
            .into_iter()
            .enumerate()
            .map(|(i, co)| Chunk {
                chunk_id: self.id_generator.new_uuid(),
                chunk_set_id,
                sequence: i as u32,
                heading: co.heading,
                text: co.text,
                char_start: co.char_start,
                char_end: co.char_end,
            })
            .collect();

        let chunk_set = ChunkSet {
            chunk_set_id,
            document_id,
            document_version,
            chunking_config: variant.config,
            created_at: occurred_at.to_string(),
        };
        self.chunk_set_repository
            .save(chunk_set, chunks.clone())
            .await?;

        Ok((chunk_set_id, chunks))
    }

    async fn find_or_create_embedding_set(
        &self,
        chunk_set_id: Uuid,
        chunks: &[Chunk],
        embedding_model: &ResolvedEmbeddingModel,
    ) -> Result<Uuid, AppError> {
        if let Some(existing) = self
            .embedding_set_repository
            .find_by(chunk_set_id, embedding_model.embedding_model_id)
            .await?
        {
            return Ok(existing.embedding_set_id);
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let vectors = self
            .embedding_service
            .embed_with_resolved(embedding_model, &texts)
            .await?;

        let embedding_set_id = self.id_generator.new_uuid();
        let occurred_at = self.clock.now();
        let embedding_set = EmbeddingSet {
            embedding_set_id,
            chunk_set_id,
            embedding_model_id: embedding_model.embedding_model_id,
            embedding_model_snapshot:
                crate::server::domain::configuration::embedding_model::EmbeddingModel {
                    embedding_model_id: embedding_model.embedding_model_id,
                    kind: embedding_model.kind,
                    model: embedding_model.model.clone(),
                    dimensions: embedding_model.dimensions,
                },
            dimensions: embedding_model.dimensions,
            created_at: occurred_at.to_string(),
        };

        let embeddings: Vec<ChunkEmbedding> = chunks
            .iter()
            .zip(vectors.iter())
            .map(|(chunk, vec)| ChunkEmbedding {
                chunk_id: chunk.chunk_id,
                embedding_set_id,
                vector: vec.clone(),
            })
            .collect();

        self.embedding_set_repository
            .save(embedding_set, embeddings)
            .await?;

        Ok(embedding_set_id)
    }

    async fn score_variant(
        &self,
        embedding_set_id: Uuid,
        questions: &[EvaluationQuestion],
        chunks: &[Chunk],
        question_embeddings: &[Vec<f32>],
        options: &EvaluationRunOptions,
    ) -> Result<(EvaluationMetrics, Vec<RetrievalTraceEntry>), AppError> {
        let chunk_by_id: std::collections::HashMap<Uuid, &Chunk> =
            chunks.iter().map(|c| (c.chunk_id, c)).collect();

        let mut recall_scores = Vec::with_capacity(questions.len());
        let mut precision_scores = Vec::with_capacity(questions.len());
        let mut iou_scores = Vec::with_capacity(questions.len());
        let mut omega_scores = Vec::with_capacity(questions.len());
        let mut traces = Vec::with_capacity(questions.len());

        for (q_idx, (question, q_emb)) in
            questions.iter().zip(question_embeddings.iter()).enumerate()
        {
            let retrieved = self
                .retriever
                .retrieve(&RetrievalQuery {
                    embedding_set_id,
                    query_vector: q_emb.clone(),
                    top_k: options.top_k,
                    min_score: options.min_score(),
                })
                .await?;

            let mut retrieved_refs = Vec::with_capacity(retrieved.len());
            let mut retrieved_chunk_ids = Vec::with_capacity(retrieved.len());
            let mut scores = Vec::with_capacity(retrieved.len());
            for r in &retrieved {
                if let Some(&chunk) = chunk_by_id.get(&r.chunk_id) {
                    retrieved_refs.push(chunk);
                    retrieved_chunk_ids.push(r.chunk_id);
                    scores.push(r.score);
                }
            }

            let (recall, precision, iou) = score_question(question, &retrieved_refs);
            let omega = precision_omega(question, chunks);

            recall_scores.push(recall);
            precision_scores.push(precision);
            iou_scores.push(iou);
            omega_scores.push(omega);

            traces.push(RetrievalTraceEntry {
                question_sequence: q_idx as u32,
                retrieved_chunk_ids,
                scores,
                recall,
                precision,
                iou,
            });
        }

        let metrics = EvaluationMetrics {
            recall_mean: mean(&recall_scores),
            recall_std: std_dev(&recall_scores),
            precision_mean: mean(&precision_scores),
            precision_std: std_dev(&precision_scores),
            iou_mean: mean(&iou_scores),
            iou_std: std_dev(&iou_scores),
            precision_omega_mean: mean(&omega_scores),
            precision_omega_std: std_dev(&omega_scores),
        };

        Ok((metrics, traces))
    }
}

#[async_trait]
impl EffectExecutor<EvaluationRunEffect> for EvaluationRunEffectExecutor {
    async fn execute(&self, effect: &EvaluationRunEffect) -> Result<(), AppError> {
        match effect {
            EvaluationRunEffect::ExecuteRun(e) => self.run(e).await,
        }
    }
}
