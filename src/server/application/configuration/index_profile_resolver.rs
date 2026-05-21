use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::indexing::{ResolvedVectorIndex, VectorIndexResolver};
use crate::server::application::AppError;
use crate::server::domain::configuration::defaults::ConfigurationDefaultsRepository;
use crate::server::domain::configuration::index_profile::IndexProfileRepository;
use crate::server::domain::connector::ConnectorRepository;

#[derive(Debug, Clone)]
pub struct ResolvedIndexProfile {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model: ResolvedEmbeddingModel,
    pub vector_index: ResolvedVectorIndex,
}

pub struct IndexProfileResolver {
    index_profile_repository: Arc<dyn IndexProfileRepository>,
    connector_repository: Arc<dyn ConnectorRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
    embedding_service: Arc<EmbeddingService>,
    vector_index_resolver: Arc<VectorIndexResolver>,
}

impl IndexProfileResolver {
    pub fn new(
        index_profile_repository: Arc<dyn IndexProfileRepository>,
        connector_repository: Arc<dyn ConnectorRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
        embedding_service: Arc<EmbeddingService>,
        vector_index_resolver: Arc<VectorIndexResolver>,
    ) -> Arc<Self> {
        Arc::new(Self {
            index_profile_repository,
            connector_repository,
            defaults,
            embedding_service,
            vector_index_resolver,
        })
    }

    pub async fn resolve_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<ResolvedIndexProfile, AppError> {
        let connector = self
            .connector_repository
            .load(connector_id)
            .await?
            .filter(|m| !m.deleted)
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))?;

        let index_profile_id = match connector.default_index_profile_id {
            Some(id) => id,
            None => self
                .defaults
                .load()
                .await?
                .index_profile_id
                .ok_or_else(|| {
                    AppError::Validation(
                        "no default index profile configured for connector or application".into(),
                    )
                })?,
        };

        self.resolve(index_profile_id).await
    }

    pub async fn resolve(&self, index_profile_id: Uuid) -> Result<ResolvedIndexProfile, AppError> {
        let ip = self
            .index_profile_repository
            .find_by_id(index_profile_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("index profile {index_profile_id} not found"))
            })?;

        let embedding_model = self
            .embedding_service
            .resolve(ip.embedding_model_id)
            .await?;
        let vector_index = self
            .vector_index_resolver
            .resolve(ip.vector_index_id)
            .await?;

        Ok(ResolvedIndexProfile {
            index_profile_id: ip.index_profile_id,
            name: ip.name,
            embedding_model,
            vector_index,
        })
    }
}
