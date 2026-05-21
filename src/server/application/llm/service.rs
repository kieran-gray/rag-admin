use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::llm::ports::{
    GenerationRequest, GenerationResponse, GenerationResponseFormat,
};
use crate::server::application::llm::GenerationClientRegistry;
use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::shared::reference_data::AiProviderKind;

#[derive(Debug, Clone)]
pub struct ResolvedGenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct GenerationPrompt {
    pub system: String,
    pub user: String,
    pub temperature: f32,
    pub response_format: GenerationResponseFormat,
}

pub struct GenerationService {
    clients: GenerationClientRegistry,
    generation_models: Arc<dyn GenerationModelRepository>,
}

impl GenerationService {
    pub fn new(
        clients: GenerationClientRegistry,
        generation_models: Arc<dyn GenerationModelRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            clients,
            generation_models,
        })
    }

    pub async fn generate(
        &self,
        generation_model_id: Uuid,
        prompt: GenerationPrompt,
    ) -> Result<GenerationResponse, AppError> {
        let resolved = self.resolve(generation_model_id).await?;
        let client = self.clients.get(&resolved.kind).ok_or_else(|| {
            AppError::Internal(format!(
                "no generation client registered for provider kind {}",
                resolved.kind.as_str()
            ))
        })?;
        client
            .generate(GenerationRequest {
                model: resolved.model,
                system: prompt.system,
                user: prompt.user,
                temperature: prompt.temperature,
                response_format: prompt.response_format,
            })
            .await
    }

    pub async fn resolve(
        &self,
        generation_model_id: Uuid,
    ) -> Result<ResolvedGenerationModel, AppError> {
        let model = self
            .generation_models
            .find_by_id(generation_model_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "generation model {generation_model_id} not registered"
                ))
            })?;
        Ok(ResolvedGenerationModel {
            generation_model_id: model.generation_model_id,
            kind: model.kind,
            model: model.model,
        })
    }
}
