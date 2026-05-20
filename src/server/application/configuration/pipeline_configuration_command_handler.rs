use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::pipeline_configuration::{
    AddPipelineConfiguration, EmbeddingModelRef, GenerationModelRef, PipelineConfigurationCatalog,
    PipelineConfigurationCatalogCommand, PipelineConfigurationCatalogEvent,
    RemovePipelineConfiguration, UpdatePipelineConfiguration, VectorIndexRef,
};
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::shared::contracts::{
    CreatePipelineConfigurationDto, PipelineConfigurationCommandDto, UpdatePipelineConfigurationDto,
};
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::CommandProcessor;

pub struct PipelineConfigurationCatalogCommandHandler {
    processor: Arc<CommandProcessor<PipelineConfigurationCatalog>>,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
}

impl PipelineConfigurationCatalogCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<PipelineConfigurationCatalog>>,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            embedding_models,
            generation_models,
            vector_indexes,
            defaults,
        })
    }

    pub async fn handle_dto(
        &self,
        command: PipelineConfigurationCommandDto,
    ) -> Result<(), AppError> {
        match command {
            PipelineConfigurationCommandDto::CreatePipelineConfiguration(d) => {
                let is_default = d.is_default;
                let cmd = self.add_command_from(d).await?;
                let appended = self
                    .dispatch(PipelineConfigurationCatalogCommand::AddPipelineConfiguration(cmd))
                    .await?;
                if is_default {
                    if let Some(id) = added_id(&appended) {
                        self.defaults.set_default_pipeline_configuration(id).await?;
                    }
                }
                Ok(())
            }
            PipelineConfigurationCommandDto::UpdatePipelineConfiguration(d) => {
                let is_default = d.is_default;
                let pipeline_configuration_id = d.pipeline_configuration_id;
                let cmd = self.update_command_from(d).await?;
                self.dispatch(
                    PipelineConfigurationCatalogCommand::UpdatePipelineConfiguration(cmd),
                )
                .await?;
                if is_default {
                    self.defaults
                        .set_default_pipeline_configuration(pipeline_configuration_id)
                        .await?;
                }
                Ok(())
            }
            PipelineConfigurationCommandDto::DeletePipelineConfiguration(d) => self
                .dispatch(
                    PipelineConfigurationCatalogCommand::RemovePipelineConfiguration(
                        RemovePipelineConfiguration {
                            pipeline_configuration_id: d.pipeline_configuration_id,
                        },
                    ),
                )
                .await
                .map(|_| ()),
        }
    }

    async fn add_command_from(
        &self,
        dto: CreatePipelineConfigurationDto,
    ) -> Result<AddPipelineConfiguration, AppError> {
        let embedding_model = self.resolve_embedding(dto.embedding_model_id).await?;
        let generation_model = self.resolve_generation(dto.generation_model_id).await?;
        let vector_index = self.resolve_vector_index(dto.vector_index_id).await?;
        Ok(AddPipelineConfiguration {
            name: dto.name,
            embedding_model,
            generation_model,
            vector_index,
        })
    }

    async fn update_command_from(
        &self,
        dto: UpdatePipelineConfigurationDto,
    ) -> Result<UpdatePipelineConfiguration, AppError> {
        let embedding_model = self.resolve_embedding(dto.embedding_model_id).await?;
        let generation_model = self.resolve_generation(dto.generation_model_id).await?;
        let vector_index = self.resolve_vector_index(dto.vector_index_id).await?;
        Ok(UpdatePipelineConfiguration {
            pipeline_configuration_id: dto.pipeline_configuration_id,
            name: dto.name,
            embedding_model,
            generation_model,
            vector_index,
        })
    }

    async fn resolve_embedding(&self, id: Uuid) -> Result<EmbeddingModelRef, AppError> {
        let model = self
            .embedding_models
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Validation(format!("embedding model {id} not found")))?;
        Ok(EmbeddingModelRef {
            embedding_model_id: model.embedding_model_id,
            dimensions: model.dimensions,
        })
    }

    async fn resolve_generation(&self, id: Uuid) -> Result<GenerationModelRef, AppError> {
        self.generation_models
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Validation(format!("generation model {id} not found")))?;
        Ok(GenerationModelRef {
            generation_model_id: id,
        })
    }

    async fn resolve_vector_index(&self, id: Uuid) -> Result<VectorIndexRef, AppError> {
        let index = self
            .vector_indexes
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Validation(format!("vector index {id} not found")))?;
        Ok(VectorIndexRef {
            vector_index_id: index.index_id,
            dimensions: index.dimensions,
        })
    }

    async fn dispatch(
        &self,
        command: PipelineConfigurationCatalogCommand,
    ) -> Result<Vec<EventEnvelope<PipelineConfigurationCatalogEvent>>, AppError> {
        let appended = self
            .processor
            .handle(PipelineConfigurationCatalog::singleton_id(), command)
            .await?;
        Ok(appended)
    }
}

fn added_id(events: &[EventEnvelope<PipelineConfigurationCatalogEvent>]) -> Option<Uuid> {
    events.iter().find_map(|e| match &e.event {
        PipelineConfigurationCatalogEvent::PipelineConfigurationAdded(added) => {
            Some(added.pipeline_configuration_id)
        }
        _ => None,
    })
}
