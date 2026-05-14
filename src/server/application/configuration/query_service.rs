use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationRepository;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::kinds::{AiProviderKind, VectorStoreKind};
use crate::server::domain::configuration::pipeline_configuration::PipelineConfigurationRepository;
use crate::server::domain::configuration::sweep_template::{
    SweepTemplateRepository, SweepTemplateRepositoryError,
};
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::shared::{
    AiProviderKindDto, ChunkingConfigurationDto, ConfigurationDto, EmbeddingModelDto,
    GenerationModelDto, PipelineConfigurationDto, SweepTemplateDto, VectorIndexDto,
    VectorStoreKindDto,
};
use uuid::Uuid;

pub struct ConfigurationQueryService {
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
}

impl ConfigurationQueryService {
    pub fn new(
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            embedding_models,
            generation_models,
            vector_indexes,
        })
    }

    pub async fn get(&self) -> Result<ConfigurationDto, AppError> {
        let embedding_models = self.embedding_models.load_all().await?;
        let generation_models = self.generation_models.load_all().await?;
        let vector_indexes = self.vector_indexes.load_all().await?;

        Ok(ConfigurationDto {
            configuration_id: Uuid::nil(),
            embedding_models: embedding_models
                .into_iter()
                .map(|m| EmbeddingModelDto {
                    embedding_model_id: m.embedding_model_id,
                    kind: ai_provider_kind_dto(m.kind),
                    model: m.model,
                    dimensions: m.dimensions,
                })
                .collect(),
            generation_models: generation_models
                .into_iter()
                .map(|m| GenerationModelDto {
                    generation_model_id: m.generation_model_id,
                    kind: ai_provider_kind_dto(m.kind),
                    model: m.model,
                })
                .collect(),
            vector_indexes: vector_indexes
                .into_iter()
                .map(|i| VectorIndexDto {
                    index_id: i.index_id,
                    kind: vector_store_kind_dto(i.kind),
                    name: i.name,
                    dimensions: i.dimensions,
                })
                .collect(),
        })
    }
}

pub struct PipelineConfigurationQueryService {
    pipeline_repository: Arc<dyn PipelineConfigurationRepository>,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
}

impl PipelineConfigurationQueryService {
    pub fn new(
        pipeline_repository: Arc<dyn PipelineConfigurationRepository>,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipeline_repository,
            embedding_models,
            generation_models,
            vector_indexes,
        })
    }

    pub async fn list(&self) -> Result<Vec<PipelineConfigurationDto>, AppError> {
        let pipelines = self.pipeline_repository.load_all().await?;
        let embedding_models = self.embedding_models.load_all().await?;
        let generation_models = self.generation_models.load_all().await?;
        let vector_indexes = self.vector_indexes.load_all().await?;

        Ok(pipelines
            .into_iter()
            .map(|pc| PipelineConfigurationDto {
                pipeline_configuration_id: pc.pipeline_configuration_id,
                name: pc.name,
                embedding_model_name: embedding_models
                    .iter()
                    .find(|m| m.embedding_model_id == pc.embedding_model_id)
                    .map(|m| m.model.clone()),
                embedding_model_id: pc.embedding_model_id,
                generation_model_name: generation_models
                    .iter()
                    .find(|m| m.generation_model_id == pc.generation_model_id)
                    .map(|m| m.model.clone()),
                generation_model_id: pc.generation_model_id,
                vector_index_name: vector_indexes
                    .iter()
                    .find(|i| i.index_id == pc.vector_index_id)
                    .map(|i| i.name.clone()),
                vector_index_id: pc.vector_index_id,
            })
            .collect())
    }
}

pub struct ChunkingConfigurationQueryService {
    repository: Arc<dyn ChunkingConfigurationRepository>,
}

impl ChunkingConfigurationQueryService {
    pub fn new(repository: Arc<dyn ChunkingConfigurationRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list(&self) -> Result<Vec<ChunkingConfigurationDto>, AppError> {
        let read_models = self.repository.load_all().await?;

        Ok(read_models
            .into_iter()
            .map(|cc| ChunkingConfigurationDto {
                chunking_configuration_id: cc.chunking_configuration_id,
                name: cc.name,
                config: cc.config,
            })
            .collect())
    }
}

pub struct SweepTemplateQueryService {
    repository: Arc<dyn SweepTemplateRepository>,
}

impl SweepTemplateQueryService {
    pub fn new(repository: Arc<dyn SweepTemplateRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list(&self) -> Result<Vec<SweepTemplateDto>, AppError> {
        let read_models = self.repository.load_all().await.map_err(|e| match e {
            SweepTemplateRepositoryError::Internal(m) => AppError::Internal(m),
        })?;

        Ok(read_models
            .into_iter()
            .map(|st| SweepTemplateDto {
                sweep_template_id: st.sweep_template_id,
                name: st.name,
                members: st.members,
                is_default: st.is_default,
            })
            .collect())
    }
}

fn ai_provider_kind_dto(kind: AiProviderKind) -> AiProviderKindDto {
    match kind {
        AiProviderKind::Cloudflare => AiProviderKindDto::Cloudflare,
        AiProviderKind::Ollama => AiProviderKindDto::Ollama,
    }
}

fn vector_store_kind_dto(kind: VectorStoreKind) -> VectorStoreKindDto {
    match kind {
        VectorStoreKind::CloudflareVectorize => VectorStoreKindDto::CloudflareVectorize,
        VectorStoreKind::Postgres => VectorStoreKindDto::Postgres,
    }
}
