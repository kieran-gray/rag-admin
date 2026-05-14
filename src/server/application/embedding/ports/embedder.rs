use async_trait::async_trait;

use crate::server::application::AppError;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed_batch(
        &self,
        model: &str,
        dimensions: u32,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError>;
}
