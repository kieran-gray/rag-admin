use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::{
    ChunkingConfigurationRepository, ChunkingConfigurationUpdate, NewChunkingConfiguration,
};
use crate::shared::{
    ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    DeleteChunkingConfigurationDto, UpdateChunkingConfigurationDto,
};

pub struct ChunkingConfigurationService {
    repository: Arc<dyn ChunkingConfigurationRepository>,
}

impl ChunkingConfigurationService {
    pub fn new(repository: Arc<dyn ChunkingConfigurationRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn handle_dto(
        &self,
        command: ChunkingConfigurationCommandDto,
    ) -> Result<(), AppError> {
        match command {
            ChunkingConfigurationCommandDto::CreateChunkingConfiguration(d) => self.create(d).await,
            ChunkingConfigurationCommandDto::UpdateChunkingConfiguration(d) => self.update(d).await,
            ChunkingConfigurationCommandDto::DeleteChunkingConfiguration(d) => self.delete(d).await,
        }
    }

    async fn create(&self, dto: CreateChunkingConfigurationDto) -> Result<(), AppError> {
        validate_non_empty("chunking configuration name", &dto.name)?;
        self.repository
            .create(NewChunkingConfiguration {
                id: Uuid::new_v4(),
                name: dto.name,
                config: dto.config,
            })
            .await?;
        Ok(())
    }

    async fn update(&self, dto: UpdateChunkingConfigurationDto) -> Result<(), AppError> {
        validate_non_empty("chunking configuration name", &dto.name)?;
        self.repository
            .update(ChunkingConfigurationUpdate {
                id: dto.chunking_configuration_id,
                name: dto.name,
                config: dto.config,
            })
            .await?;
        Ok(())
    }

    async fn delete(&self, dto: DeleteChunkingConfigurationDto) -> Result<(), AppError> {
        self.repository
            .delete(dto.chunking_configuration_id)
            .await?;
        Ok(())
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(format!("{field} cannot be empty")));
    }
    Ok(())
}
