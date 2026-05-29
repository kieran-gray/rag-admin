use async_trait::async_trait;

use crate::server::application::AppError;

use super::model::RawGuide;

#[async_trait]
pub trait GuideRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<RawGuide>, AppError>;

    async fn get(&self, slug: &str) -> Result<Option<RawGuide>, AppError>;
}
