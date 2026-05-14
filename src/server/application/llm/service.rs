use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::{
    ChatClient, ChatRequest, ChatResponse, ChatResponseFormat,
};
use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::server::domain::configuration::kinds::AiProviderKind;

#[derive(Debug, Clone)]
pub struct ResolvedGenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ChatPrompt {
    pub system: String,
    pub user: String,
    pub temperature: f32,
    pub response_format: ChatResponseFormat,
}

pub struct ChatService {
    clients: HashMap<AiProviderKind, Arc<dyn ChatClient>>,
    generation_models: Arc<dyn GenerationModelRepository>,
}

impl ChatService {
    pub fn new(
        clients: HashMap<AiProviderKind, Arc<dyn ChatClient>>,
        generation_models: Arc<dyn GenerationModelRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            clients,
            generation_models,
        })
    }

    pub async fn chat(
        &self,
        generation_model_id: Uuid,
        prompt: ChatPrompt,
    ) -> Result<ChatResponse, AppError> {
        let resolved = self.resolve(generation_model_id).await?;
        let client = self.clients.get(&resolved.kind).ok_or_else(|| {
            AppError::Internal(format!(
                "no chat client registered for provider kind {}",
                resolved.kind.as_str()
            ))
        })?;
        client
            .chat(ChatRequest {
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
