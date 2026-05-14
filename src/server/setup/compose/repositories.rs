use std::sync::Arc;

use sqlx::PgPool;

use crate::server::application::indexing::ports::KvStore;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationRepository;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::pipeline_configuration::PipelineConfigurationRepository;
use crate::server::domain::configuration::sweep_template::SweepTemplateRepository;
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::server::domain::embedding_set::repository::EmbeddingSetRepository;
use crate::server::domain::evaluation::dataset::repository::EvaluationDatasetRepository;
use crate::server::domain::evaluation::run::repository::EvaluationRunRepository;
use crate::server::domain::indexing::repository::IndexingRepository;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::event_sourcing::checkpoint::CheckpointRepository;
use crate::server::infrastructure::clients::CloudflareApi;
use crate::server::infrastructure::configuration::{
    PostgresChunkingConfigurationRepository, PostgresEmbeddingModelRepository,
    PostgresGenerationModelRepository, PostgresPipelineConfigurationRepository,
    PostgresSweepTemplateRepository, PostgresVectorIndexRepository,
};
use crate::server::infrastructure::evaluation::{
    PostgresEvaluationDatasetRepository, PostgresEvaluationRunRepository,
};
use crate::server::infrastructure::event_sourcing::PostgresCheckpointRepository;
use crate::server::infrastructure::indexing::PostgresIndexingRepository;
use crate::server::infrastructure::kv::{CloudflareKvStore, PostgresKvStore};
use crate::server::infrastructure::source_document::{
    PostgresBlobStore, PostgresChunkSetRepository, PostgresEmbeddingSetRepository,
    PostgresSourceDocumentRepository,
};
use crate::server::setup::config::{Config, KvBackend};
use crate::server::setup::exceptions::SetupError;

pub struct Repositories {
    pub embedding_model: Arc<dyn EmbeddingModelRepository>,
    pub generation_model: Arc<dyn GenerationModelRepository>,
    pub vector_index: Arc<dyn VectorIndexRepository>,
    pub pipeline_configuration: Arc<dyn PipelineConfigurationRepository>,
    pub chunking_configuration: Arc<dyn ChunkingConfigurationRepository>,
    pub sweep_template: Arc<dyn SweepTemplateRepository>,
    pub source_document: Arc<dyn SourceDocumentRepository>,
    pub indexing: Arc<dyn IndexingRepository>,
    pub evaluation_dataset: Arc<dyn EvaluationDatasetRepository>,
    pub evaluation_run: Arc<dyn EvaluationRunRepository>,
    pub blob_store: Arc<dyn BlobStore>,
    pub chunk_set: Arc<dyn ChunkSetRepository>,
    pub embedding_set: Arc<dyn EmbeddingSetRepository>,
    pub checkpoint: Arc<dyn CheckpointRepository>,
    pub kv_store: Arc<dyn KvStore>,
}

pub fn build_repositories(
    pool: &PgPool,
    config: &Config,
    cf_api: &Arc<CloudflareApi>,
) -> Result<Repositories, SetupError> {
    let kv_store: Arc<dyn KvStore> = match config.kv_backend {
        KvBackend::Cloudflare => {
            let namespace_id = config.cloudflare.kv_namespace_id.clone().ok_or_else(|| {
                SetupError::Internal(
                    "KV_BACKEND=cloudflare but CLOUDFLARE_KV_NAMESPACE_ID is unset".into(),
                )
            })?;
            CloudflareKvStore::new(Arc::clone(cf_api), namespace_id)
        }
        KvBackend::Postgres => PostgresKvStore::new(pool.clone()),
    };

    Ok(Repositories {
        embedding_model: Arc::new(PostgresEmbeddingModelRepository::new(pool.clone())),
        generation_model: Arc::new(PostgresGenerationModelRepository::new(pool.clone())),
        vector_index: Arc::new(PostgresVectorIndexRepository::new(pool.clone())),
        pipeline_configuration: Arc::new(PostgresPipelineConfigurationRepository::new(
            pool.clone(),
        )),
        chunking_configuration: Arc::new(PostgresChunkingConfigurationRepository::new(
            pool.clone(),
        )),
        sweep_template: Arc::new(PostgresSweepTemplateRepository::new(pool.clone())),
        source_document: Arc::new(PostgresSourceDocumentRepository::new(pool.clone())),
        indexing: Arc::new(PostgresIndexingRepository::new(pool.clone())),
        evaluation_dataset: Arc::new(PostgresEvaluationDatasetRepository::new(pool.clone())),
        evaluation_run: Arc::new(PostgresEvaluationRunRepository::new(pool.clone())),
        blob_store: Arc::new(PostgresBlobStore::new(pool.clone())),
        chunk_set: Arc::new(PostgresChunkSetRepository::new(pool.clone())),
        embedding_set: Arc::new(PostgresEmbeddingSetRepository::new(pool.clone())),
        checkpoint: Arc::new(PostgresCheckpointRepository::new(pool.clone())),
        kv_store,
    })
}
