use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::chunk_set::ChunkSetCommandHandler;
use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::chunk_set::aggregate::derive_chunk_set_id;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::configuration::embedding_model::EmbeddingModel;
use crate::server::domain::embedding_set::entity::{ChunkEmbedding, EmbeddingSet};
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::shared::value_objects::ChunkingConfig as DomainChunkingConfig;
use crate::shared::ChunkingConfig;

use super::run_context::{PreparedVariant, RunContext};

pub struct VariantIndexer {
    chunker_registry: Arc<ChunkerRegistry>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
    embedding_service: Arc<EmbeddingService>,
    embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl VariantIndexer {
    pub fn new(
        chunker_registry: Arc<ChunkerRegistry>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        chunk_set_command_handler: Arc<ChunkSetCommandHandler>,
        embedding_service: Arc<EmbeddingService>,
        embedding_set_repository: Arc<dyn EmbeddingSetRepository>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            chunker_registry,
            chunk_set_repository,
            chunk_set_command_handler,
            embedding_service,
            embedding_set_repository,
            clock,
            id_generator,
        })
    }

    pub async fn prepare(
        &self,
        ctx: &RunContext,
        label: String,
        config: ChunkingConfig,
    ) -> Result<PreparedVariant, AppError> {
        let (chunk_set_id, chunks) = self
            .find_or_create_chunk_set(
                ctx.document_id,
                ctx.document_version,
                &ctx.plain_text,
                &config,
            )
            .await?;

        let chunk_token_counts = self.count_tokens(&chunks)?;
        let embedding_set_id = self
            .find_or_create_embedding_set(chunk_set_id, &chunks, &ctx.embedding_model)
            .await?;

        Ok(PreparedVariant {
            label,
            config,
            chunk_set_id,
            embedding_set_id,
            chunks,
            chunk_token_counts,
        })
    }

    fn count_tokens(&self, chunks: &[Chunk]) -> Result<HashMap<Uuid, u32>, AppError> {
        let tokenizer = self.chunker_registry.tokenizer();
        let mut map = HashMap::with_capacity(chunks.len());
        for chunk in chunks {
            map.insert(chunk.chunk_id, tokenizer.count(&chunk.text)?);
        }
        Ok(map)
    }

    async fn find_or_create_chunk_set(
        &self,
        document_id: Uuid,
        document_version: u32,
        plain_text: &str,
        config: &ChunkingConfig,
    ) -> Result<(Uuid, Vec<Chunk>), AppError> {
        let domain_config: DomainChunkingConfig = (*config).into();
        let chunk_set_id = derive_chunk_set_id(document_id, document_version, &domain_config);

        if let Some(summary) = self.chunk_set_repository.load_summary(chunk_set_id).await? {
            if summary.chunk_count > 0 {
                let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
                return Ok((chunk_set_id, chunks));
            }
        }

        let chunk_outputs = self
            .chunker_registry
            .chunk_markdown(config, plain_text)
            .await
            .map_err(|e| AppError::Internal(format!("chunking failed: {e}")))?;

        let chunks: Vec<Chunk> = chunk_outputs
            .into_iter()
            .enumerate()
            .map(|(i, co)| Chunk {
                chunk_id: self.id_generator.new_uuid(),
                sequence: i as u32,
                heading: co.heading,
                text: co.text,
                char_start: co.char_start,
                char_end: co.char_end,
            })
            .collect();

        self.chunk_set_command_handler
            .create(
                chunk_set_id,
                document_id,
                document_version,
                domain_config,
                chunks.clone(),
            )
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
}
