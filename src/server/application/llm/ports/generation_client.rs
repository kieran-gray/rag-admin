use async_trait::async_trait;
use futures_util::stream::{self, BoxStream, StreamExt};

use crate::server::application::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationResponseFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub model: String,
    pub system: String,
    pub user: String,
    pub temperature: f32,
    pub response_format: GenerationResponseFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationResponse {
    pub content: String,
}

pub type GenerationTokenStream = BoxStream<'static, Result<String, AppError>>;

#[async_trait]
pub trait GenerationClient: Send + Sync {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AppError>;

    async fn generate_stream(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationTokenStream, AppError> {
        let response = self.generate(request).await?;
        Ok(stream::once(async move { Ok(response.content) }).boxed())
    }
}
