use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::{PipelineResolver, ResolvedPipeline};
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::ports::KvStore;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{ActivityRegistry, AppError, InternalLogEvent, Job, JobRegistry};
use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::embedding_set::entity::{ChunkEmbedding, EmbeddingSet};
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::commands::{
    CompleteChunking, CompleteEmbedding, CompleteIndexing, FailIngestion, IndexingCommand,
};
use crate::server::domain::indexing::read_model::IndexingReadModel;
use crate::server::domain::indexing::repository::IndexingRepository;
use crate::server::domain::indexing::status::IngestStage;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::VectorRecord;
use crate::server::event_sourcing::command_processor::CommandProcessor;
use crate::server::event_sourcing::process_manager::EffectExecutor;

use super::indexing::{
    ExecuteChunkingEffect, ExecuteEmbeddingEffect, ExecuteIndexingEffect, IndexingEffect,
};

const EMBED_BATCH: usize = 50;
const UPSERT_BATCH: usize = 100;
const TAIL_CLEANUP_WINDOW: u32 = 512;

pub struct IndexingEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    indexing_repository: Arc<dyn IndexingRepository>,
    blob_store: Arc<dyn BlobStore>,
    chunker_registry: Arc<ChunkerRegistry>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    embedding_service: Arc<EmbeddingService>,
    embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
    vector_index_resolver: Arc<VectorIndexResolver>,
    pipeline_resolver: Arc<PipelineResolver>,
    kv_store: Arc<dyn KvStore>,
    command_processor: Arc<CommandProcessor<Indexing>>,
    job_registry: Arc<JobRegistry>,
    activity_registry: Arc<ActivityRegistry>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl IndexingEffectExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        indexing_repository: Arc<dyn IndexingRepository>,
        blob_store: Arc<dyn BlobStore>,
        chunker_registry: Arc<ChunkerRegistry>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        embedding_service: Arc<EmbeddingService>,
        embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
        vector_index_resolver: Arc<VectorIndexResolver>,
        pipeline_resolver: Arc<PipelineResolver>,
        kv_store: Arc<dyn KvStore>,
        command_processor: Arc<CommandProcessor<Indexing>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            indexing_repository,
            blob_store,
            chunker_registry,
            chunk_set_repository,
            embedding_service,
            embedding_set_repository,
            vector_index_resolver,
            pipeline_resolver,
            kv_store,
            command_processor,
            job_registry,
            activity_registry,
            clock,
            id_generator,
        })
    }

    async fn open_job(&self, indexing_id: Uuid) -> (String, Arc<Job>) {
        let (job_id, job) = self.job_registry.create().await;
        let stream_url = format!("/api/job/logs/{job_id}");
        self.activity_registry
            .attach_stream(indexing_id, stream_url)
            .await;
        (job_id, job)
    }

    async fn finish_with_failure(
        &self,
        indexing_id: Uuid,
        stage: IngestStage,
        job: &Arc<Job>,
        err: &AppError,
    ) {
        job.emit(
            InternalLogEvent::error(format!("{stage} stage failed: {err}"))
                .with_meta("stage", json!(stage.to_string()))
                .with_meta("error", json!(err.to_string())),
        )
        .await;
        let _ = self
            .command_processor
            .handle(
                indexing_id,
                IndexingCommand::FailIngestion(FailIngestion {
                    stage,
                    reason: err.to_string(),
                    occurred_at: self.clock.now(),
                }),
            )
            .await;
    }

    async fn load_indexing(&self, indexing_id: Uuid) -> Result<IndexingReadModel, AppError> {
        self.indexing_repository
            .load(indexing_id)
            .await
            .map_err(|e| AppError::Internal(format!("load indexing: {e}")))?
            .ok_or_else(|| AppError::NotFound(format!("indexing {indexing_id}")))
    }

    async fn load_pipeline(
        &self,
        indexing: &IndexingReadModel,
    ) -> Result<ResolvedPipeline, AppError> {
        self.pipeline_resolver
            .resolve(indexing.pipeline_configuration_id)
            .await
    }

    async fn execute_chunking(&self, effect: &ExecuteChunkingEffect) -> Result<(), AppError> {
        let (_job_id, job) = self.open_job(effect.indexing_id).await;
        match self.run_chunking(effect.indexing_id, &job).await {
            Ok(()) => {
                job.finish().await;
                Ok(())
            }
            Err(e) => {
                self.finish_with_failure(effect.indexing_id, IngestStage::Chunking, &job, &e)
                    .await;
                job.finish().await;
                Err(e)
            }
        }
    }

    async fn execute_embedding(&self, effect: &ExecuteEmbeddingEffect) -> Result<(), AppError> {
        let (_job_id, job) = self.open_job(effect.indexing_id).await;
        match self.run_embedding(effect.indexing_id, &job).await {
            Ok(()) => {
                job.finish().await;
                Ok(())
            }
            Err(e) => {
                self.finish_with_failure(effect.indexing_id, IngestStage::Embedding, &job, &e)
                    .await;
                job.finish().await;
                Err(e)
            }
        }
    }

    async fn execute_indexing(&self, effect: &ExecuteIndexingEffect) -> Result<(), AppError> {
        let (_job_id, job) = self.open_job(effect.indexing_id).await;
        match self.run_indexing(effect.indexing_id, &job).await {
            Ok(()) => {
                job.finish().await;
                Ok(())
            }
            Err(e) => {
                self.finish_with_failure(effect.indexing_id, IngestStage::Indexing, &job, &e)
                    .await;
                job.finish().await;
                Err(e)
            }
        }
    }

    async fn run_chunking(&self, indexing_id: Uuid, job: &Arc<Job>) -> Result<(), AppError> {
        let indexing = self.load_indexing(indexing_id).await?;
        if indexing.chunk_set_id.is_some() && !is_requeue_safe(&indexing) {
            job.emit(InternalLogEvent::info(
                "Chunking already complete for this indexing; nothing to do.",
            ))
            .await;
            return Ok(());
        }

        let document = self
            .source_document_repository
            .load(indexing.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", indexing.document_id)))?;

        let bytes = self.blob_store.get(&document.latest_content_hash).await?;
        let markdown = String::from_utf8(bytes).map_err(|e| {
            AppError::Internal(format!(
                "content for {} is not utf-8: {e}",
                document.document_id
            ))
        })?;

        job.emit(
            InternalLogEvent::info(format!(
                "Chunking '{}' with {}",
                document.latest_content_hash.as_hex(),
                indexing.chunking_config.describe(),
            ))
            .with_meta("indexing_id", json!(indexing_id.to_string()))
            .with_meta("document_id", json!(indexing.document_id.to_string()))
            .with_meta(
                "chunking_config",
                json!(indexing.chunking_config.describe()),
            ),
        )
        .await;

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(&indexing.chunking_config, &markdown)
            .await
            .map_err(|e| AppError::Internal(format!("chunking failed: {e}")))?;

        let chunk_count = chunk_outputs.len() as u32;
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
            document_id: indexing.document_id,
            document_version: indexing.document_version,
            chunking_config: indexing.chunking_config,
            created_at: occurred_at.to_string(),
        };
        self.chunk_set_repository.save(chunk_set, chunks).await?;

        self.command_processor
            .handle(
                indexing_id,
                IndexingCommand::CompleteChunking(CompleteChunking {
                    chunk_set_id,
                    chunk_count,
                    occurred_at,
                }),
            )
            .await?;

        job.emit(
            InternalLogEvent::success(format!("Chunking complete · {chunk_count} chunks"))
                .with_meta("chunk_count", json!(chunk_count))
                .with_meta("chunk_set_id", json!(chunk_set_id.to_string())),
        )
        .await;
        Ok(())
    }

    async fn run_embedding(&self, indexing_id: Uuid, job: &Arc<Job>) -> Result<(), AppError> {
        let indexing = self.load_indexing(indexing_id).await?;
        let chunk_set_id = indexing.chunk_set_id.ok_or_else(|| {
            AppError::Validation(
                "embedding requested before chunking completed; run chunking first".into(),
            )
        })?;

        if indexing.embedding_set_id.is_some() && indexing.status.is_at_least_embedding() {
            job.emit(InternalLogEvent::info(
                "Embedding already complete for this indexing; nothing to do.",
            ))
            .await;
            return Ok(());
        }

        let pipeline = self.load_pipeline(&indexing).await?;
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        let embedding_model = &pipeline.embedding_model;

        let embedding_set_id = if let Some(existing) = self
            .embedding_set_repository
            .find_by(chunk_set_id, embedding_model.embedding_model_id)
            .await?
        {
            job.emit(
                InternalLogEvent::info(format!(
                    "Reusing existing embedding set {}",
                    existing.embedding_set_id
                ))
                .with_meta(
                    "embedding_set_id",
                    json!(existing.embedding_set_id.to_string()),
                ),
            )
            .await;
            existing.embedding_set_id
        } else {
            job.emit(
                InternalLogEvent::info(format!(
                    "Embedding {} chunks via {} ({} dims)",
                    chunks.len(),
                    embedding_model.model,
                    embedding_model.dimensions,
                ))
                .with_meta("chunk_count", json!(chunks.len()))
                .with_meta("embedding_model", json!(embedding_model.model))
                .with_meta("dimensions", json!(embedding_model.dimensions)),
            )
            .await;

            let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
            let total_batches = texts.len().div_ceil(EMBED_BATCH);
            let mut all_vectors: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
            for (i, batch) in texts.chunks(EMBED_BATCH).enumerate() {
                job.emit(
                    InternalLogEvent::info(format!(
                        "Embedding batch {}/{} ({} chunks)",
                        i + 1,
                        total_batches,
                        batch.len(),
                    ))
                    .with_meta("batch", json!(i + 1))
                    .with_meta("total_batches", json!(total_batches))
                    .with_meta("batch_size", json!(batch.len())),
                )
                .await;
                let vecs = self
                    .embedding_service
                    .embed_with_resolved(embedding_model, batch)
                    .await?;
                all_vectors.extend(vecs);
            }

            let new_set_id = self.id_generator.new_uuid();
            let occurred_at = self.clock.now();
            let embedding_set = EmbeddingSet {
                embedding_set_id: new_set_id,
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
                .zip(all_vectors)
                .map(|(chunk, vector)| ChunkEmbedding {
                    chunk_id: chunk.chunk_id,
                    embedding_set_id: new_set_id,
                    vector,
                })
                .collect();
            self.embedding_set_repository
                .save(embedding_set, embeddings)
                .await?;
            new_set_id
        };

        let occurred_at = self.clock.now();
        self.command_processor
            .handle(
                indexing_id,
                IndexingCommand::CompleteEmbedding(CompleteEmbedding {
                    embedding_set_id,
                    occurred_at,
                }),
            )
            .await?;
        job.emit(
            InternalLogEvent::success("Embedding complete")
                .with_meta("embedding_set_id", json!(embedding_set_id.to_string())),
        )
        .await;
        Ok(())
    }

    async fn run_indexing(&self, indexing_id: Uuid, job: &Arc<Job>) -> Result<(), AppError> {
        let indexing = self.load_indexing(indexing_id).await?;
        let embedding_set_id = indexing.embedding_set_id.ok_or_else(|| {
            AppError::Validation(
                "indexing requested before embedding completed; run embedding first".into(),
            )
        })?;
        let chunk_set_id = indexing.chunk_set_id.ok_or_else(|| {
            AppError::Validation("indexing requested without a chunk_set; restart chunking".into())
        })?;
        if indexing.status.is_indexed() {
            job.emit(InternalLogEvent::info(
                "Indexing already complete; nothing to upsert.",
            ))
            .await;
            return Ok(());
        }

        let pipeline = self.load_pipeline(&indexing).await?;
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        let embeddings = self
            .embedding_set_repository
            .load_embeddings(embedding_set_id)
            .await?;
        let chunk_map: std::collections::HashMap<Uuid, &Chunk> =
            chunks.iter().map(|c| (c.chunk_id, c)).collect();

        let document_id = indexing.document_id;
        let pipeline_configuration_id = indexing.pipeline_configuration_id;
        let document_version = indexing.document_version;

        let document = self
            .source_document_repository
            .load(document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {document_id}")))?;
        let post_slug = document.source_ref.natural_key().to_string();
        let post_version = document_version.to_string();

        let doc_id_hex = uuid_hex(document_id);
        let pipeline_hex_short = &uuid_hex(pipeline_configuration_id)[..8];

        let records: Vec<VectorRecord> = embeddings
            .iter()
            .filter_map(|e| chunk_map.get(&e.chunk_id).map(|c| (e, *c)))
            .map(|(e, chunk)| VectorRecord {
                id: vector_id(&doc_id_hex, pipeline_hex_short, chunk.sequence),
                values: e.vector.clone(),
                metadata: json!({
                    "document_id": document_id.to_string(),
                    "document_version": document_version,
                    "pipeline_configuration_id": pipeline_configuration_id.to_string(),
                    "chunk_id": chunk.chunk_id.to_string(),
                    "chunk_set_id": chunk_set_id.to_string(),
                    "post_slug": post_slug,
                    "post_version": post_version,
                    "heading": chunk.heading,
                    "text": chunk.text,
                    "char_start": chunk.char_start,
                    "char_end": chunk.char_end,
                }),
            })
            .collect();

        let vector_index = self.vector_index_resolver.build(&pipeline.vector_index)?;
        let vector_count = records.len() as u32;
        let vector_index_name = pipeline.vector_index.name.clone();

        job.emit(
            InternalLogEvent::info(format!(
                "Upserting {} vectors to '{}'",
                vector_count, vector_index_name
            ))
            .with_meta("vector_count", json!(vector_count))
            .with_meta("vector_index", json!(vector_index_name)),
        )
        .await;

        let total_batches = records.len().div_ceil(UPSERT_BATCH);
        for (i, batch) in records.chunks(UPSERT_BATCH).enumerate() {
            job.emit(
                InternalLogEvent::info(format!(
                    "Upsert batch {}/{} ({} records)",
                    i + 1,
                    total_batches,
                    batch.len(),
                ))
                .with_meta("batch", json!(i + 1))
                .with_meta("total_batches", json!(total_batches))
                .with_meta("batch_size", json!(batch.len())),
            )
            .await;
            vector_index.upsert(batch).await?;
        }

        let tail_ids: Vec<String> = (vector_count..vector_count + TAIL_CLEANUP_WINDOW)
            .map(|seq| vector_id(&doc_id_hex, pipeline_hex_short, seq))
            .collect();
        if let Err(e) = vector_index.delete(&tail_ids).await {
            job.emit(
                InternalLogEvent::info(format!("Tail cleanup delete skipped: {e}"))
                    .with_meta("tail_window", json!(TAIL_CLEANUP_WINDOW)),
            )
            .await;
        }

        let kv_key = format!("post_version:{post_slug}");
        let kv_value = json!({ "v": post_version });
        if let Err(e) = self.kv_store.put_json(&kv_key, &kv_value).await {
            job.emit(
                InternalLogEvent::error(format!("KV post_version write failed: {e}"))
                    .with_meta("kv_key", json!(kv_key))
                    .with_meta("error", json!(e.to_string())),
            )
            .await;
            return Err(e);
        }

        let occurred_at = self.clock.now();
        self.command_processor
            .handle(
                indexing_id,
                IndexingCommand::CompleteIndexing(CompleteIndexing {
                    vector_count,
                    occurred_at,
                }),
            )
            .await?;
        job.emit(
            InternalLogEvent::success(format!(
                "Upsert complete · {vector_count} vectors \u{2192} '{vector_index_name}'"
            ))
            .with_meta("vector_count", json!(vector_count))
            .with_meta("vector_index", json!(vector_index_name)),
        )
        .await;
        Ok(())
    }
}

fn uuid_hex(id: Uuid) -> String {
    let mut buf = [0u8; 32];
    id.as_simple().encode_lower(&mut buf);
    String::from_utf8(buf.to_vec()).expect("hex uuid is utf-8")
}

fn vector_id(doc_id_hex: &str, pipeline_hex_short: &str, sequence: u32) -> String {
    format!("{doc_id_hex}:{pipeline_hex_short}:{sequence}")
}

/// A "safe to requeue" indexing is one where the operator explicitly asked
/// to re-chunk despite chunking already being recorded. The aggregate doesn't
/// reset state on `ChunkingRequeued`, but we still rerun the work because
/// the operator asked for it explicitly — the previous chunk set is left
/// orphaned, the new one is created and recorded via `CompleteChunking`.
fn is_requeue_safe(_indexing: &IndexingReadModel) -> bool {
    // Today: never auto-skip. The policy fires the effect only on
    // operator-initiated `ChunkingRequeued` (or initial `IngestRequested`),
    // so reaching here means the operator wants the work to run.
    true
}

#[async_trait]
impl EffectExecutor<IndexingEffect> for IndexingEffectExecutor {
    async fn execute(&self, effect: &IndexingEffect) -> Result<(), AppError> {
        match effect {
            IndexingEffect::ExecuteChunking(e) => self.execute_chunking(e).await,
            IndexingEffect::ExecuteEmbedding(e) => self.execute_embedding(e).await,
            IndexingEffect::ExecuteIndexing(e) => self.execute_indexing(e).await,
        }
    }
}
