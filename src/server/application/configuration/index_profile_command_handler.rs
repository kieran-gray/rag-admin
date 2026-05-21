use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::EmbeddingModelRepository;
use crate::server::domain::configuration::index_profile::{
    AddIndexProfile, EmbeddingModelRef, IndexProfileCatalog, IndexProfileCatalogCommand,
    IndexProfileCatalogEvent, RemoveIndexProfile, UpdateIndexProfile, VectorIndexRef,
};
use crate::server::domain::configuration::vector_index::VectorIndexRepository;
use crate::shared::contracts::{
    CreateIndexProfileDto, IndexProfileCommandDto, UpdateIndexProfileDto,
};
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::CommandProcessor;

pub struct IndexProfileCatalogCommandHandler {
    processor: Arc<CommandProcessor<IndexProfileCatalog>>,
    embedding_models: Arc<dyn EmbeddingModelRepository>,
    vector_indexes: Arc<dyn VectorIndexRepository>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
}

impl IndexProfileCatalogCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<IndexProfileCatalog>>,
        embedding_models: Arc<dyn EmbeddingModelRepository>,
        vector_indexes: Arc<dyn VectorIndexRepository>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            embedding_models,
            vector_indexes,
            defaults,
        })
    }

    pub async fn handle_dto(&self, command: IndexProfileCommandDto) -> Result<(), AppError> {
        match command {
            IndexProfileCommandDto::CreateIndexProfile(d) => {
                let is_default = d.is_default;
                let cmd = self.add_command_from(d).await?;
                let appended = self
                    .dispatch(IndexProfileCatalogCommand::AddIndexProfile(cmd))
                    .await?;
                if is_default {
                    if let Some(id) = added_id(&appended) {
                        self.defaults.set_default_index_profile(id).await?;
                    }
                }
                Ok(())
            }
            IndexProfileCommandDto::UpdateIndexProfile(d) => {
                let is_default = d.is_default;
                let index_profile_id = d.index_profile_id;
                let cmd = self.update_command_from(d).await?;
                self.dispatch(IndexProfileCatalogCommand::UpdateIndexProfile(cmd))
                    .await?;
                if is_default {
                    self.defaults
                        .set_default_index_profile(index_profile_id)
                        .await?;
                }
                Ok(())
            }
            IndexProfileCommandDto::DeleteIndexProfile(d) => self
                .dispatch(IndexProfileCatalogCommand::RemoveIndexProfile(
                    RemoveIndexProfile {
                        index_profile_id: d.index_profile_id,
                    },
                ))
                .await
                .map(|_| ()),
        }
    }

    async fn add_command_from(
        &self,
        dto: CreateIndexProfileDto,
    ) -> Result<AddIndexProfile, AppError> {
        let embedding_model = self.resolve_embedding(dto.embedding_model_id).await?;
        let vector_index = self.resolve_vector_index(dto.vector_index_id).await?;
        Ok(AddIndexProfile {
            name: dto.name,
            embedding_model,
            vector_index,
        })
    }

    async fn update_command_from(
        &self,
        dto: UpdateIndexProfileDto,
    ) -> Result<UpdateIndexProfile, AppError> {
        let embedding_model = self.resolve_embedding(dto.embedding_model_id).await?;
        let vector_index = self.resolve_vector_index(dto.vector_index_id).await?;
        Ok(UpdateIndexProfile {
            index_profile_id: dto.index_profile_id,
            name: dto.name,
            embedding_model,
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
        command: IndexProfileCatalogCommand,
    ) -> Result<Vec<EventEnvelope<IndexProfileCatalogEvent>>, AppError> {
        let appended = self
            .processor
            .handle(IndexProfileCatalog::singleton_id(), command)
            .await?;
        Ok(appended)
    }
}

fn added_id(events: &[EventEnvelope<IndexProfileCatalogEvent>]) -> Option<Uuid> {
    events.iter().find_map(|e| match &e.event {
        IndexProfileCatalogEvent::IndexProfileAdded(added) => Some(added.index_profile_id),
        _ => None,
    })
}
