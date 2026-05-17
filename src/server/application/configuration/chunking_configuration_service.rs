use std::sync::Arc;

use uuid::Uuid;

use crate::contracts::{
    ChunkingConfigurationCommandDto, CreateChunkingConfigurationDto,
    DeleteChunkingConfigurationDto, UpdateChunkingConfigurationDto,
};
use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::AppError;
use crate::server::domain::configuration::chunking_configuration::{
    ChunkingConfigurationRepository, ChunkingConfigurationUpdate, NewChunkingConfiguration,
};

pub struct ChunkingConfigurationService {
    repository: Arc<dyn ChunkingConfigurationRepository>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
}

impl ChunkingConfigurationService {
    pub fn new(
        repository: Arc<dyn ChunkingConfigurationRepository>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            repository,
            defaults,
        })
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
        let id = Uuid::new_v4();
        self.repository
            .create(NewChunkingConfiguration {
                id,
                name: dto.name,
                config: dto.config,
            })
            .await?;
        if dto.is_default {
            self.defaults.set_default_chunking_configuration(id).await?;
        }
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
        if dto.is_default {
            self.defaults
                .set_default_chunking_configuration(dto.chunking_configuration_id)
                .await?;
        }
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
