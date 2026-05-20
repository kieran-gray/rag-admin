use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::indexing::{ResolvedVectorIndex, VectorIndexResolver};
use crate::server::application::llm::{GenerationService, ResolvedGenerationModel};
use crate::server::application::AppError;
use crate::server::domain::configuration::pipeline_configuration::PipelineConfigurationRepository;
use crate::server::domain::connector::ConnectorRepository;

#[derive(Debug, Clone)]
pub struct ResolvedPipeline {
    pub pipeline_configuration_id: Uuid,
    pub name: String,
    pub embedding_model: ResolvedEmbeddingModel,
    pub generation_model: ResolvedGenerationModel,
    pub vector_index: ResolvedVectorIndex,
}

pub struct PipelineResolver {
    pipeline_repository: Arc<dyn PipelineConfigurationRepository>,
    connector_repository: Arc<dyn ConnectorRepository>,
    embedding_service: Arc<EmbeddingService>,
    generation_service: Arc<GenerationService>,
    vector_index_resolver: Arc<VectorIndexResolver>,
}

impl PipelineResolver {
    pub fn new(
        pipeline_repository: Arc<dyn PipelineConfigurationRepository>,
        connector_repository: Arc<dyn ConnectorRepository>,
        embedding_service: Arc<EmbeddingService>,
        generation_service: Arc<GenerationService>,
        vector_index_resolver: Arc<VectorIndexResolver>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipeline_repository,
            connector_repository,
            embedding_service,
            generation_service,
            vector_index_resolver,
        })
    }

    pub async fn resolve_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<ResolvedPipeline, AppError> {
        let connector = self
            .connector_repository
            .load(connector_id)
            .await?
            .filter(|m| !m.deleted)
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))?;

        let pipeline_id = match connector.default_pipeline_configuration_id {
            Some(id) => id,
            None => {
                self.pipeline_repository
                    .find_default()
                    .await?
                    .ok_or_else(|| {
                        AppError::Validation(
                            "no default pipeline configured for connector or application".into(),
                        )
                    })?
                    .pipeline_configuration_id
            }
        };

        self.resolve(pipeline_id).await
    }

    pub async fn resolve(
        &self,
        pipeline_configuration_id: Uuid,
    ) -> Result<ResolvedPipeline, AppError> {
        let pc = self
            .pipeline_repository
            .find_by_id(pipeline_configuration_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "pipeline configuration {pipeline_configuration_id} not found"
                ))
            })?;

        let embedding_model = self
            .embedding_service
            .resolve(pc.embedding_model_id)
            .await?;
        let generation_model = self
            .generation_service
            .resolve(pc.generation_model_id)
            .await?;
        let vector_index = self
            .vector_index_resolver
            .resolve(pc.vector_index_id)
            .await?;

        Ok(ResolvedPipeline {
            pipeline_configuration_id: pc.pipeline_configuration_id,
            name: pc.name,
            embedding_model,
            generation_model,
            vector_index,
        })
    }
}
