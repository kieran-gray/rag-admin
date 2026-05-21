use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::index_profile_resolver::{
    IndexProfileResolver, ResolvedIndexProfile,
};
use crate::server::application::llm::{GenerationService, ResolvedGenerationModel};
use crate::server::application::AppError;
use crate::server::domain::configuration::defaults::ConfigurationDefaultsRepository;
use crate::server::domain::configuration::retrieval_profile::RetrievalProfileRepository;
use crate::server::domain::connector::ConnectorRepository;

#[derive(Debug, Clone)]
pub struct ResolvedRetrievalProfile {
    pub retrieval_profile_id: Uuid,
    pub name: String,
    pub index_profile: ResolvedIndexProfile,
    pub generation_model: ResolvedGenerationModel,
    pub reranker_model_id: Option<Uuid>,
    pub default_top_k: u32,
    pub default_min_score_milli: u32,
}

pub struct RetrievalProfileResolver {
    retrieval_profile_repository: Arc<dyn RetrievalProfileRepository>,
    connector_repository: Arc<dyn ConnectorRepository>,
    defaults: Arc<dyn ConfigurationDefaultsRepository>,
    index_profile_resolver: Arc<IndexProfileResolver>,
    generation_service: Arc<GenerationService>,
}

impl RetrievalProfileResolver {
    pub fn new(
        retrieval_profile_repository: Arc<dyn RetrievalProfileRepository>,
        connector_repository: Arc<dyn ConnectorRepository>,
        defaults: Arc<dyn ConfigurationDefaultsRepository>,
        index_profile_resolver: Arc<IndexProfileResolver>,
        generation_service: Arc<GenerationService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            retrieval_profile_repository,
            connector_repository,
            defaults,
            index_profile_resolver,
            generation_service,
        })
    }

    pub async fn resolve_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<ResolvedRetrievalProfile, AppError> {
        let connector = self
            .connector_repository
            .load(connector_id)
            .await?
            .filter(|m| !m.deleted)
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))?;

        let retrieval_profile_id = match connector.default_retrieval_profile_id {
            Some(id) => id,
            None => self
                .defaults
                .load()
                .await?
                .retrieval_profile_id
                .ok_or_else(|| {
                    AppError::Validation(
                        "no default retrieval profile configured for connector or application"
                            .into(),
                    )
                })?,
        };

        self.resolve(retrieval_profile_id).await
    }

    pub async fn resolve(
        &self,
        retrieval_profile_id: Uuid,
    ) -> Result<ResolvedRetrievalProfile, AppError> {
        let rp = self
            .retrieval_profile_repository
            .find_by_id(retrieval_profile_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "retrieval profile {retrieval_profile_id} not found"
                ))
            })?;

        let index_profile = self
            .index_profile_resolver
            .resolve(rp.index_profile_id)
            .await?;
        let generation_model = self
            .generation_service
            .resolve(rp.generation_model_id)
            .await?;

        Ok(ResolvedRetrievalProfile {
            retrieval_profile_id: rp.retrieval_profile_id,
            name: rp.name,
            index_profile,
            generation_model,
            reranker_model_id: rp.reranker_model_id,
            default_top_k: rp.default_top_k,
            default_min_score_milli: rp.default_min_score_milli,
        })
    }
}
