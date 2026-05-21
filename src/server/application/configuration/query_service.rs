use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::ChunkingConfigurationRepository;
use crate::server::domain::configuration::defaults::ConfigurationDefaultsRepository;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::index_profile::IndexProfileRepository;
use crate::server::domain::configuration::retrieval_profile::RetrievalProfileRepository;
use crate::server::domain::configuration::sweep_template::{
    SweepTemplateRepository, SweepTemplateRepositoryError,
};
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::server::domain::connector::ConnectorRepository;
use crate::shared::contracts::{
    ChunkingConfigurationDto, ConfigurationDto, EmbeddingModelDto, GenerationModelDto,
    IndexProfileDto, RetrievalProfileDto, SweepTemplateDto, VectorIndexDto,
};
use crate::shared::ChunkingConfig;
use uuid::Uuid;

pub struct ConfigurationQueryService {
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
    index_profiles: Arc<dyn IndexProfileRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
}

impl ConfigurationQueryService {
    pub fn new(
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
        index_profiles: Arc<dyn IndexProfileRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            embedding_models,
            generation_models,
            vector_indexes,
            index_profiles,
            defaults,
        })
    }

    pub async fn get(&self) -> Result<ConfigurationDto, AppError> {
        let embedding_models = self.embedding_models.load_all().await?;
        let generation_models = self.generation_models.load_all().await?;
        let vector_indexes = self.vector_indexes.load_all().await?;
        let index_profiles = self.index_profiles.load_all().await?;
        let default_id = self.defaults.load().await?.index_profile_id;

        Ok(ConfigurationDto {
            configuration_id: Uuid::nil(),
            embedding_models: embedding_models
                .iter()
                .map(|m| EmbeddingModelDto {
                    embedding_model_id: m.embedding_model_id,
                    kind: m.kind,
                    model: m.model.clone(),
                    dimensions: m.dimensions,
                })
                .collect(),
            generation_models: generation_models
                .into_iter()
                .map(|m| GenerationModelDto {
                    generation_model_id: m.generation_model_id,
                    kind: m.kind,
                    model: m.model,
                })
                .collect(),
            vector_indexes: vector_indexes
                .iter()
                .map(|i| VectorIndexDto {
                    index_id: i.index_id,
                    kind: i.kind,
                    name: i.name.clone(),
                    dimensions: i.dimensions,
                })
                .collect(),
            index_profiles: index_profiles
                .into_iter()
                .map(|ip| IndexProfileDto {
                    embedding_model_name: embedding_models
                        .iter()
                        .find(|m| m.embedding_model_id == ip.embedding_model_id)
                        .map(|m| m.model.clone()),
                    vector_index_name: vector_indexes
                        .iter()
                        .find(|i| i.index_id == ip.vector_index_id)
                        .map(|i| i.name.clone()),
                    is_default: default_id == Some(ip.index_profile_id),
                    index_profile_id: ip.index_profile_id,
                    name: ip.name,
                    embedding_model_id: ip.embedding_model_id,
                    vector_index_id: ip.vector_index_id,
                    dimensions: ip.dimensions,
                })
                .collect(),
        })
    }
}

pub struct IndexProfileQueryService {
    index_profiles: Arc<dyn IndexProfileRepository>,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
}

impl IndexProfileQueryService {
    pub fn new(
        index_profiles: Arc<dyn IndexProfileRepository>,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            index_profiles,
            embedding_models,
            vector_indexes,
            defaults,
        })
    }

    pub async fn list(&self) -> Result<Vec<IndexProfileDto>, AppError> {
        let profiles = self.index_profiles.load_all().await?;
        let embedding_models = self.embedding_models.load_all().await?;
        let vector_indexes = self.vector_indexes.load_all().await?;
        let default_id = self.defaults.load().await?.index_profile_id;

        Ok(profiles
            .into_iter()
            .map(|ip| IndexProfileDto {
                embedding_model_name: embedding_models
                    .iter()
                    .find(|m| m.embedding_model_id == ip.embedding_model_id)
                    .map(|m| m.model.clone()),
                vector_index_name: vector_indexes
                    .iter()
                    .find(|i| i.index_id == ip.vector_index_id)
                    .map(|i| i.name.clone()),
                is_default: default_id == Some(ip.index_profile_id),
                index_profile_id: ip.index_profile_id,
                name: ip.name,
                embedding_model_id: ip.embedding_model_id,
                vector_index_id: ip.vector_index_id,
                dimensions: ip.dimensions,
            })
            .collect())
    }

    pub async fn find_default_id(&self) -> Result<Option<Uuid>, AppError> {
        Ok(self.defaults.load().await?.index_profile_id)
    }
}

pub struct RetrievalProfileQueryService {
    retrieval_profiles: Arc<dyn RetrievalProfileRepository>,
    index_profiles: Arc<dyn IndexProfileRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
}

impl RetrievalProfileQueryService {
    pub fn new(
        retrieval_profiles: Arc<dyn RetrievalProfileRepository>,
        index_profiles: Arc<dyn IndexProfileRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            retrieval_profiles,
            index_profiles,
            generation_models,
            defaults,
        })
    }

    pub async fn list(&self) -> Result<Vec<RetrievalProfileDto>, AppError> {
        let profiles = self.retrieval_profiles.load_all().await?;
        let index_profiles = self.index_profiles.load_all().await?;
        let generation_models = self.generation_models.load_all().await?;
        let default_id = self.defaults.load().await?.retrieval_profile_id;

        Ok(profiles
            .into_iter()
            .map(|rp| RetrievalProfileDto {
                index_profile_name: index_profiles
                    .iter()
                    .find(|ip| ip.index_profile_id == rp.index_profile_id)
                    .map(|ip| ip.name.clone()),
                generation_model_name: generation_models
                    .iter()
                    .find(|m| m.generation_model_id == rp.generation_model_id)
                    .map(|m| m.model.clone()),
                is_default: default_id == Some(rp.retrieval_profile_id),
                retrieval_profile_id: rp.retrieval_profile_id,
                name: rp.name,
                index_profile_id: rp.index_profile_id,
                generation_model_id: rp.generation_model_id,
                reranker_model_id: rp.reranker_model_id,
                default_top_k: rp.default_top_k,
                default_min_score_milli: rp.default_min_score_milli,
            })
            .collect())
    }

    pub async fn find_default_id(&self) -> Result<Option<Uuid>, AppError> {
        Ok(self.defaults.load().await?.retrieval_profile_id)
    }
}

pub struct ChunkingConfigurationQueryService {
    repository: Arc<dyn ChunkingConfigurationRepository>,
    connector_repository: Arc<dyn ConnectorRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
}

impl ChunkingConfigurationQueryService {
    pub fn new(
        repository: Arc<dyn ChunkingConfigurationRepository>,
        connector_repository: Arc<dyn ConnectorRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            repository,
            connector_repository,
            defaults,
        })
    }

    pub async fn list(&self) -> Result<Vec<ChunkingConfigurationDto>, AppError> {
        let entries = self.repository.load_all().await?;
        let default_id = self.defaults.load().await?.chunking_configuration_id;

        Ok(entries
            .into_iter()
            .map(|cc| ChunkingConfigurationDto {
                chunking_configuration_id: cc.chunking_configuration_id,
                name: cc.name,
                config: cc.config.into(),
                is_default: default_id == Some(cc.chunking_configuration_id),
            })
            .collect())
    }

    pub async fn resolve_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<ChunkingConfig, AppError> {
        let connector = self
            .connector_repository
            .load(connector_id)
            .await?
            .filter(|m| !m.deleted)
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))?;

        let chunking_id = match connector.default_chunking_configuration_id {
            Some(id) => id,
            None => self
                .defaults
                .load()
                .await?
                .chunking_configuration_id
                .ok_or_else(|| {
                    AppError::Validation(
                        "no default chunking configured for connector or application".into(),
                    )
                })?,
        };

        let entry = self
            .repository
            .find_by_id(chunking_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("chunking configuration {chunking_id} not found"))
            })?;

        Ok(entry.config.into())
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
