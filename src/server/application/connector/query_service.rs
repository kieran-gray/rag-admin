use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::connector::{ConnectorConfig, ConnectorReadModel, ConnectorRepository};
use crate::shared::contracts::connector::{ConnectorConfigDto, ConnectorDto, SitemapConfigDto};

pub struct ConnectorQueryService {
    repository: Arc<dyn ConnectorRepository>,
}

impl ConnectorQueryService {
    pub fn new(repository: Arc<dyn ConnectorRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn list(&self) -> Result<Vec<ConnectorDto>, AppError> {
        let models = self.repository.list_active().await?;
        Ok(models.into_iter().map(read_model_to_dto).collect())
    }

    pub async fn get(&self, connector_id: Uuid) -> Result<Option<ConnectorDto>, AppError> {
        let model = self.repository.load(connector_id).await?;
        Ok(model.filter(|m| !m.deleted).map(read_model_to_dto))
    }

    pub async fn get_config(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorConfig>, AppError> {
        let model = self.repository.load(connector_id).await?;
        Ok(model.filter(|m| !m.deleted).map(|m| m.config))
    }

    pub async fn get_read_model(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorReadModel>, AppError> {
        let model = self.repository.load(connector_id).await?;
        Ok(model.filter(|m| !m.deleted))
    }
}

fn read_model_to_dto(model: ConnectorReadModel) -> ConnectorDto {
    ConnectorDto {
        connector_id: model.connector_id,
        name: model.name,
        config: config_domain_to_dto(model.config),
        default_index_profile_id: model.default_index_profile_id,
        default_chunking_configuration_id: model.default_chunking_configuration_id,
        default_retrieval_profile_id: model.default_retrieval_profile_id,
    }
}

fn config_domain_to_dto(config: ConnectorConfig) -> ConnectorConfigDto {
    match config {
        ConnectorConfig::Sitemap(c) => ConnectorConfigDto::Sitemap(SitemapConfigDto {
            url: c.url,
            include_patterns: c.include_patterns,
            exclude_patterns: c.exclude_patterns,
        }),
    }
}
