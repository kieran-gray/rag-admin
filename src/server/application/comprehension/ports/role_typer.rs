use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::comprehension::role::SuggestedRole;

#[async_trait]
pub trait RoleTyper: Send + Sync {
    async fn suggest(
        &self,
        generation_model_id: Uuid,
        document_sample: &str,
    ) -> Result<Vec<SuggestedRole>, AppError>;
}
