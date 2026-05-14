use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::entity::GenerationModel;

#[derive(Debug, Error)]
pub enum GenerationModelRepositoryError {
    #[error("generation model repository error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait GenerationModelRepository: Send + Sync {
    async fn load_all(&self) -> Result<Vec<GenerationModel>, GenerationModelRepositoryError>;

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<GenerationModel>, GenerationModelRepositoryError>;

    async fn save(&self, model: GenerationModel) -> Result<(), GenerationModelRepositoryError>;

    async fn delete(&self, model_id: Uuid) -> Result<(), GenerationModelRepositoryError>;
}
