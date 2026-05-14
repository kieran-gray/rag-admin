use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::indexing::{ResolvedVectorIndex, VectorIndexResolver};
use crate::server::application::llm::{ChatService, ResolvedGenerationModel};
use crate::server::application::AppError;
use crate::server::domain::configuration::pipeline_configuration::PipelineConfigurationRepository;

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
    embedding_service: Arc<EmbeddingService>,
    chat_service: Arc<ChatService>,
    vector_index_resolver: Arc<VectorIndexResolver>,
}

impl PipelineResolver {
    pub fn new(
        pipeline_repository: Arc<dyn PipelineConfigurationRepository>,
        embedding_service: Arc<EmbeddingService>,
        chat_service: Arc<ChatService>,
        vector_index_resolver: Arc<VectorIndexResolver>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipeline_repository,
            embedding_service,
            chat_service,
            vector_index_resolver,
        })
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
        let generation_model = self.chat_service.resolve(pc.generation_model_id).await?;
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
