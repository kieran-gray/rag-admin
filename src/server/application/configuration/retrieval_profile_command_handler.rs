use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::index_profile::IndexProfileRepository;
use crate::server::domain::configuration::retrieval_profile::{
    AddRetrievalProfile, GenerationModelRef, IndexProfileRef, RemoveRetrievalProfile,
    RetrievalProfileCatalog, RetrievalProfileCatalogCommand, RetrievalProfileCatalogEvent,
    UpdateRetrievalProfile,
};
use crate::shared::contracts::{
    CreateRetrievalProfileDto, RetrievalProfileCommandDto, UpdateRetrievalProfileDto,
};
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::CommandProcessor;

pub struct RetrievalProfileCatalogCommandHandler {
    processor: Arc<CommandProcessor<RetrievalProfileCatalog>>,
    index_profiles: Arc<dyn IndexProfileRepository>,
    generation_models: Arc<dyn GenerationModelRepository>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
}

impl RetrievalProfileCatalogCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<RetrievalProfileCatalog>>,
        index_profiles: Arc<dyn IndexProfileRepository>,
        generation_models: Arc<dyn GenerationModelRepository>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            index_profiles,
            generation_models,
            defaults,
        })
    }

    pub async fn handle_dto(&self, command: RetrievalProfileCommandDto) -> Result<(), AppError> {
        match command {
            RetrievalProfileCommandDto::CreateRetrievalProfile(d) => {
                let is_default = d.is_default;
                let cmd = self.add_command_from(d).await?;
                let appended = self
                    .dispatch(RetrievalProfileCatalogCommand::AddRetrievalProfile(cmd))
                    .await?;
                if is_default {
                    if let Some(id) = added_id(&appended) {
                        self.defaults.set_default_retrieval_profile(id).await?;
                    }
                }
                Ok(())
            }
            RetrievalProfileCommandDto::UpdateRetrievalProfile(d) => {
                let is_default = d.is_default;
                let retrieval_profile_id = d.retrieval_profile_id;
                let cmd = self.update_command_from(d).await?;
                self.dispatch(RetrievalProfileCatalogCommand::UpdateRetrievalProfile(cmd))
                    .await?;
                if is_default {
                    self.defaults
                        .set_default_retrieval_profile(retrieval_profile_id)
                        .await?;
                }
                Ok(())
            }
            RetrievalProfileCommandDto::DeleteRetrievalProfile(d) => self
                .dispatch(RetrievalProfileCatalogCommand::RemoveRetrievalProfile(
                    RemoveRetrievalProfile {
                        retrieval_profile_id: d.retrieval_profile_id,
                    },
                ))
                .await
                .map(|_| ()),
        }
    }

    async fn add_command_from(
        &self,
        dto: CreateRetrievalProfileDto,
    ) -> Result<AddRetrievalProfile, AppError> {
        let index_profile = self.resolve_index_profile(dto.index_profile_id).await?;
        let generation_model = self.resolve_generation(dto.generation_model_id).await?;
        Ok(AddRetrievalProfile {
            name: dto.name,
            index_profile,
            generation_model,
            reranker_model_id: dto.reranker_model_id,
            default_top_k: dto.default_top_k,
            default_min_score_milli: dto.default_min_score_milli,
        })
    }

    async fn update_command_from(
        &self,
        dto: UpdateRetrievalProfileDto,
    ) -> Result<UpdateRetrievalProfile, AppError> {
        let index_profile = self.resolve_index_profile(dto.index_profile_id).await?;
        let generation_model = self.resolve_generation(dto.generation_model_id).await?;
        Ok(UpdateRetrievalProfile {
            retrieval_profile_id: dto.retrieval_profile_id,
            name: dto.name,
            index_profile,
            generation_model,
            reranker_model_id: dto.reranker_model_id,
            default_top_k: dto.default_top_k,
            default_min_score_milli: dto.default_min_score_milli,
        })
    }

    async fn resolve_index_profile(&self, id: Uuid) -> Result<IndexProfileRef, AppError> {
        self.index_profiles
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Validation(format!("index profile {id} not found")))?;
        Ok(IndexProfileRef {
            index_profile_id: id,
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

    async fn dispatch(
        &self,
        command: RetrievalProfileCatalogCommand,
    ) -> Result<Vec<EventEnvelope<RetrievalProfileCatalogEvent>>, AppError> {
        let appended = self
            .processor
            .handle(RetrievalProfileCatalog::singleton_id(), command)
            .await?;
        Ok(appended)
    }
}

fn added_id(events: &[EventEnvelope<RetrievalProfileCatalogEvent>]) -> Option<Uuid> {
    events.iter().find_map(|e| match &e.event {
        RetrievalProfileCatalogEvent::RetrievalProfileAdded(added) => {
            Some(added.retrieval_profile_id)
        }
        _ => None,
    })
}
