use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::pipeline_configuration::{
    NewPipelineConfiguration, PipelineConfigurationRepository, PipelineConfigurationUpdate,
};
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::shared::{
    CreatePipelineConfigurationDto, DeletePipelineConfigurationDto,
    PipelineConfigurationCommandDto, UpdatePipelineConfigurationDto,
};

pub struct PipelineConfigurationService {
    repository: Arc<dyn PipelineConfigurationRepository>,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
}

impl PipelineConfigurationService {
    pub fn new(
        repository: Arc<dyn PipelineConfigurationRepository>,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            repository,
            embedding_models,
            vector_indexes,
        })
    }

    pub async fn handle_dto(
        &self,
        command: PipelineConfigurationCommandDto,
    ) -> Result<(), AppError> {
        match command {
            PipelineConfigurationCommandDto::CreatePipelineConfiguration(d) => self.create(d).await,
            PipelineConfigurationCommandDto::UpdatePipelineConfiguration(d) => self.update(d).await,
            PipelineConfigurationCommandDto::DeletePipelineConfiguration(d) => self.delete(d).await,
        }
    }

    async fn create(&self, dto: CreatePipelineConfigurationDto) -> Result<(), AppError> {
        validate_non_empty("pipeline configuration name", &dto.name)?;
        self.validate_compatibility(dto.embedding_model_id, dto.vector_index_id)
            .await?;
        self.repository
            .create(NewPipelineConfiguration {
                id: Uuid::new_v4(),
                name: dto.name,
                embedding_model_id: dto.embedding_model_id,
                generation_model_id: dto.generation_model_id,
                vector_index_id: dto.vector_index_id,
            })
            .await?;
        Ok(())
    }

    async fn update(&self, dto: UpdatePipelineConfigurationDto) -> Result<(), AppError> {
        validate_non_empty("pipeline configuration name", &dto.name)?;
        self.validate_compatibility(dto.embedding_model_id, dto.vector_index_id)
            .await?;
        self.repository
            .update(PipelineConfigurationUpdate {
                id: dto.pipeline_configuration_id,
                name: dto.name,
                embedding_model_id: dto.embedding_model_id,
                generation_model_id: dto.generation_model_id,
                vector_index_id: dto.vector_index_id,
            })
            .await?;
        Ok(())
    }

    async fn delete(&self, dto: DeletePipelineConfigurationDto) -> Result<(), AppError> {
        self.repository
            .delete(dto.pipeline_configuration_id)
            .await?;
        Ok(())
    }

    async fn validate_compatibility(
        &self,
        embedding_model_id: Uuid,
        vector_index_id: Uuid,
    ) -> Result<(), AppError> {
        let embedding_model = self
            .embedding_models
            .find_by_id(embedding_model_id)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!("embedding model {embedding_model_id} not found"))
            })?;
        let vector_index = self
            .vector_indexes
            .find_by_id(vector_index_id)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!("vector index {vector_index_id} not found"))
            })?;
        if embedding_model.dimensions != vector_index.dimensions {
            return Err(AppError::Validation(format!(
                "embedding model '{}' has {} dimensions but vector index '{}' has {}",
                embedding_model.model,
                embedding_model.dimensions,
                vector_index.name,
                vector_index.dimensions,
            )));
        }
        Ok(())
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(format!("{field} cannot be empty")));
    }
    Ok(())
}
