use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::configuration::{IndexProfileResolver, ResolvedIndexProfile};
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::ports::KvStore;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::{
    ActivityRegistry, AppError, InternalLogEvent, Job, JobIdStrategy, JobRegistry, JobSession,
};
use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::configuration::embedding_model::EmbeddingModel;
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
use crate::shared::ChunkingConfig as ChunkingConfigDto;
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::process_manager::{EffectError, EffectExecutor};

use super::indexing::{
    ExecuteChunkingEffect, ExecuteEmbeddingEffect, ExecuteIndexingEffect, ExecuteRemovalEffect,
    IndexingEffect,
};

const EMBED_BATCH: usize = 50;
const UPSERT_BATCH: usize = 100;
const TAIL_CLEANUP_WINDOW: u32 = 100;
const REMOVAL_SWEEP_WINDOW: u32 = 512;

pub struct IndexingEffectExecutor {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    indexing_repository: Arc<dyn IndexingRepository>,
    blob_store: Arc<dyn BlobStore>,
    chunker_registry: Arc<ChunkerRegistry>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    embedding_service: Arc<EmbeddingService>,
    embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
    vector_index_resolver: Arc<VectorIndexResolver>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    kv_store: Arc<dyn KvStore>,
    command_processor: Arc<CommandProcessor<Indexing>>,
    session: JobSession<Indexing>,
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
        index_profile_resolver: Arc<IndexProfileResolver>,
        kv_store: Arc<dyn KvStore>,
        command_processor: Arc<CommandProcessor<Indexing>>,
        job_registry: Arc<JobRegistry>,
        activity_registry: Arc<ActivityRegistry>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        let session = JobSession::new(
            job_registry,
            activity_registry,
            Arc::clone(&command_processor),
            Arc::clone(&clock),
        );
        Arc::new(Self {
            source_document_repository,
            indexing_repository,
            blob_store,
            chunker_registry,
            chunk_set_repository,
            embedding_service,
            embedding_set_repository,
            vector_index_resolver,
            index_profile_resolver,
            kv_store,
            command_processor,
            session,
            clock,
            id_generator,
        })
    }

    fn fail_for(&self, stage: IngestStage) -> impl FnOnce(String) -> IndexingCommand + use<> {
        let clock = Arc::clone(&self.clock);
        move |reason| {
            IndexingCommand::FailIngestion(FailIngestion {
                stage,
                reason,
                occurred_at: clock.now(),
            })
        }
    }

    async fn load_indexing(&self, indexing_id: Uuid) -> Result<IndexingReadModel, AppError> {
        self.indexing_repository
            .load(indexing_id)
            .await
            .map_err(|e| AppError::Internal(format!("load indexing: {e}")))?
            .ok_or_else(|| AppError::NotFound(format!("indexing {indexing_id}")))
    }

    async fn load_index_profile(
        &self,
        indexing: &IndexingReadModel,
    ) -> Result<ResolvedIndexProfile, AppError> {
        self.index_profile_resolver
            .resolve(indexing.index_profile_id)
            .await
    }

    async fn build_index_scope(
        &self,
        indexing: &IndexingReadModel,
        index_profile: &ResolvedIndexProfile,
    ) -> Result<IndexScope, AppError> {
        let document = self
            .source_document_repository
            .load(indexing.document_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {}", indexing.document_id)))?;
        let source_slug = document
            .latest_metadata
            .slug()
            .map_or_else(|| document.source_ref.natural_key(), str::to_owned);
        Ok(IndexScope::new(
            indexing.document_id,
            indexing.index_profile_id,
            index_profile.vector_index.name.clone(),
            source_slug,
        ))
    }

    async fn execute_chunking(&self, effect: &ExecuteChunkingEffect) -> Result<(), AppError> {
        let indexing_id = effect.indexing_id;
        self.session
            .run(
                indexing_id,
                JobIdStrategy::Fresh,
                "chunking stage failed",
                self.fail_for(IngestStage::Chunking),
                |job| self.run_chunking(indexing_id, job),
            )
            .await
    }

    async fn execute_embedding(&self, effect: &ExecuteEmbeddingEffect) -> Result<(), AppError> {
        let indexing_id = effect.indexing_id;
        self.session
            .run(
                indexing_id,
                JobIdStrategy::Fresh,
                "embedding stage failed",
                self.fail_for(IngestStage::Embedding),
                |job| self.run_embedding(indexing_id, job),
            )
            .await
    }

    async fn execute_indexing(&self, effect: &ExecuteIndexingEffect) -> Result<(), AppError> {
        let indexing_id = effect.indexing_id;
        self.session
            .run(
                indexing_id,
                JobIdStrategy::Fresh,
                "indexing stage failed",
                self.fail_for(IngestStage::Indexing),
                |job| self.run_indexing(indexing_id, job),
            )
            .await
    }

    async fn execute_removal(&self, effect: &ExecuteRemovalEffect) -> Result<(), AppError> {
        let indexing = self.load_indexing(effect.indexing_id).await?;
        let index_profile = self.load_index_profile(&indexing).await?;
        let scope = self.build_index_scope(&indexing, &index_profile).await?;

        let ids = scope.vector_ids(0..REMOVAL_SWEEP_WINDOW);
        let vector_index = self
            .vector_index_resolver
            .build(&index_profile.vector_index)?;
        vector_index.delete(&ids).await?;

        self.kv_store.delete(&scope.kv_key()).await?;
        Ok(())
    }

    async fn run_chunking(&self, indexing_id: Uuid, job: Arc<Job>) -> Result<(), AppError> {
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

        let chunking_dto: ChunkingConfigDto = indexing.chunking_config.into();
        job.emit(
            InternalLogEvent::info(format!(
                "Chunking '{}' with {}",
                document.latest_content_hash.as_hex(),
                chunking_dto.describe(),
            ))
            .with_meta("indexing_id", json!(indexing_id.to_string()))
            .with_meta("document_id", json!(indexing.document_id.to_string()))
            .with_meta("chunking_config", json!(chunking_dto.describe())),
        )
        .await;

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(&chunking_dto, &markdown)
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

    async fn run_embedding(&self, indexing_id: Uuid, job: Arc<Job>) -> Result<(), AppError> {
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

        let index_profile = self.load_index_profile(&indexing).await?;
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        let embedding_model = &index_profile.embedding_model;

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
                embedding_model_snapshot: EmbeddingModel {
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

    async fn run_indexing(&self, indexing_id: Uuid, job: Arc<Job>) -> Result<(), AppError> {
        let indexing = self.load_indexing(indexing_id).await?;
        let embedding_set_id = indexing.embedding_set_id.ok_or_else(|| {
            AppError::Validation(
                "indexing requested before embedding completed; run embedding first".into(),
            )
        })?;
        let chunk_set_id = indexing.chunk_set_id.ok_or_else(|| {
            AppError::Validation("indexing requested without a chunk_set; restart chunking".into())
        })?;

        let index_profile = self.load_index_profile(&indexing).await?;
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        let embeddings = self
            .embedding_set_repository
            .load_embeddings(embedding_set_id)
            .await?;
        let chunk_map: HashMap<Uuid, &Chunk> = chunks.iter().map(|c| (c.chunk_id, c)).collect();

        let scope = self.build_index_scope(&indexing, &index_profile).await?;
        let source_version = indexing.document_version.to_string();

        let records: Vec<VectorRecord> = embeddings
            .iter()
            .filter_map(|e| chunk_map.get(&e.chunk_id).map(|c| (e, *c)))
            .map(|(e, chunk)| {
                let references = extract_markdown_references(&chunk.text);
                let reference_titles: Vec<&str> =
                    references.iter().map(|r| r.title.as_str()).collect();
                let reference_urls: Vec<&str> = references.iter().map(|r| r.url.as_str()).collect();
                VectorRecord {
                    id: scope.vector_id(chunk.sequence),
                    values: e.vector.clone(),
                    metadata: json!({
                        "document_id": indexing.document_id.to_string(),
                        "document_version": indexing.document_version,
                        "index_profile_id": indexing.index_profile_id.to_string(),
                        "chunk_id": chunk.sequence,
                        "chunk_uuid": chunk.chunk_id.to_string(),
                        "chunk_set_id": chunk_set_id.to_string(),
                        "source_slug": scope.source_slug,
                        "source_version": source_version,
                        "heading": chunk.heading,
                        "text": chunk.text,
                        "char_start": chunk.char_start,
                        "char_end": chunk.char_end,
                        "reference_titles": reference_titles,
                        "reference_urls": reference_urls,
                    }),
                }
            })
            .collect();

        let vector_index = self
            .vector_index_resolver
            .build(&index_profile.vector_index)?;
        let vector_count = records.len() as u32;

        job.emit(
            InternalLogEvent::info(format!(
                "Upserting {} vectors to '{}'",
                vector_count, scope.vector_index_name
            ))
            .with_meta("vector_count", json!(vector_count))
            .with_meta("vector_index", json!(scope.vector_index_name)),
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

        let tail_ids = scope.vector_ids(vector_count..vector_count + TAIL_CLEANUP_WINDOW);
        if let Err(e) = vector_index.delete(&tail_ids).await {
            job.emit(
                InternalLogEvent::info(format!("Tail cleanup delete skipped: {e}"))
                    .with_meta("tail_window", json!(TAIL_CLEANUP_WINDOW)),
            )
            .await;
        }

        let kv_key = scope.kv_key();
        let kv_value = json!({ "v": source_version });
        if let Err(e) = self.kv_store.put_json(&kv_key, &kv_value).await {
            job.emit(
                InternalLogEvent::error(format!("KV source_version write failed: {e}"))
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
                "Upsert complete · {} vectors \u{2192} '{}'",
                vector_count, scope.vector_index_name
            ))
            .with_meta("vector_count", json!(vector_count))
            .with_meta("vector_index", json!(scope.vector_index_name)),
        )
        .await;
        Ok(())
    }
}

fn uuid_hex(id: Uuid) -> String {
    id.as_simple().to_string()
}

struct Reference {
    title: String,
    url: String,
}

fn extract_markdown_references(text: &str) -> Vec<Reference> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let Some(rel) = bytes
            .get(i..)
            .and_then(|tail| tail.iter().position(|&b| b == b'['))
        else {
            break;
        };
        let open = i + rel;
        if open > 0 && bytes.get(open - 1) == Some(&b'!') {
            i = open + 1;
            continue;
        }
        if open > 0 && bytes.get(open - 1) == Some(&b'\\') {
            i = open + 1;
            continue;
        }

        let Some(close_rel) = bytes
            .get(open + 1..)
            .and_then(|tail| tail.iter().position(|&b| b == b']'))
        else {
            break;
        };
        let close = open + 1 + close_rel;
        if bytes.get(close + 1) != Some(&b'(') {
            i = close + 1;
            continue;
        }

        let Some(paren_close_rel) = bytes
            .get(close + 2..)
            .and_then(|tail| tail.iter().position(|&b| b == b')'))
        else {
            break;
        };
        let paren_close = close + 2 + paren_close_rel;

        let title = text.get(open + 1..close).unwrap_or("").trim().to_string();
        let target = text
            .get(close + 2..paren_close)
            .unwrap_or("")
            .trim()
            .to_string();
        let url = strip_link_title(&target).trim().to_string();
        if !title.is_empty() && is_http_url(&url) && seen_urls.insert(url.clone()) {
            out.push(Reference { title, url });
        }
        i = paren_close + 1;
    }
    out
}

fn strip_link_title(target: &str) -> &str {
    target.split_whitespace().next().unwrap_or("")
}

fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

struct IndexScope {
    doc_id_hex: String,
    index_profile_hex_short: String,
    vector_index_name: String,
    source_slug: String,
}

impl IndexScope {
    fn new(
        document_id: Uuid,
        index_profile_id: Uuid,
        vector_index_name: String,
        source_slug: String,
    ) -> Self {
        let index_profile_hex_full = uuid_hex(index_profile_id);
        let index_profile_hex_short = index_profile_hex_full.chars().take(8).collect();
        Self {
            doc_id_hex: uuid_hex(document_id),
            index_profile_hex_short,
            vector_index_name,
            source_slug,
        }
    }

    fn vector_id(&self, sequence: u32) -> String {
        format!(
            "{}:{}:{}",
            self.doc_id_hex, self.index_profile_hex_short, sequence
        )
    }

    fn vector_ids(&self, sequences: Range<u32>) -> Vec<String> {
        sequences.map(|seq| self.vector_id(seq)).collect()
    }

    fn kv_key(&self) -> String {
        format!(
            "source_version:{}:{}",
            self.vector_index_name, self.source_slug
        )
    }
}

fn is_requeue_safe(_indexing: &IndexingReadModel) -> bool {
    true
}

#[async_trait]
impl EffectExecutor<IndexingEffect> for IndexingEffectExecutor {
    async fn execute(&self, effect: &IndexingEffect) -> Result<(), EffectError> {
        let result = match effect {
            IndexingEffect::ExecuteChunking(e) => self.execute_chunking(e).await,
            IndexingEffect::ExecuteEmbedding(e) => self.execute_embedding(e).await,
            IndexingEffect::ExecuteIndexing(e) => self.execute_indexing(e).await,
            IndexingEffect::ExecuteRemoval(e) => self.execute_removal(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}

#[cfg(test)]
mod reference_tests {
    use super::extract_markdown_references;

    fn collect(text: &str) -> Vec<(String, String)> {
        extract_markdown_references(text)
            .into_iter()
            .map(|r| (r.title, r.url))
            .collect()
    }

    #[test]
    fn extracts_basic_markdown_links() {
        let text =
            "See [Martin Fowler](https://martinfowler.com/eaaDev/EventSourcing.html) for details.";
        assert_eq!(
            collect(text),
            vec![(
                "Martin Fowler".to_string(),
                "https://martinfowler.com/eaaDev/EventSourcing.html".to_string()
            )]
        );
    }

    #[test]
    fn skips_image_markdown() {
        let text = "![alt](https://example.com/image.png)";
        assert!(collect(text).is_empty());
    }

    #[test]
    fn skips_relative_urls() {
        let text = "Internal link: [home](/posts/foo).";
        assert!(collect(text).is_empty());
    }

    #[test]
    fn dedupes_repeated_urls() {
        let text = "First [Fowler](https://example.com) and again [Fowler](https://example.com).";
        assert_eq!(collect(text).len(), 1);
    }

    #[test]
    fn handles_link_with_title_attribute() {
        let text = r#"[Docs](https://example.com "Title")"#;
        assert_eq!(
            collect(text),
            vec![("Docs".to_string(), "https://example.com".to_string())]
        );
    }
}
