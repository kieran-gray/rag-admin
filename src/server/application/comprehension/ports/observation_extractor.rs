use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::comprehension::observation::Observation;

pub struct ObservationContext<'a> {
    pub document_id: Uuid,
    pub chunk: &'a Chunk,
    pub suggested_roles_summary: &'a str,
}

#[async_trait]
pub trait ObservationExtractor: Send + Sync {
    async fn extract(
        &self,
        generation_model_id: Uuid,
        ctx: ObservationContext<'_>,
    ) -> Result<Vec<Observation>, AppError>;
}
