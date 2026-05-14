use async_trait::async_trait;

use crate::server::application::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatResponseFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub user: String,
    pub temperature: f32,
    pub response_format: ChatResponseFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    pub content: String,
}

#[async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, AppError>;
}
