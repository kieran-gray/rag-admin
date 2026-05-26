use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::thread::Thread;

pub struct InsightSynthRequest<'a> {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub observations: &'a [Observation],
    pub threads: &'a [Thread],
}

#[async_trait]
pub trait InsightSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        generation_model_id: Uuid,
        req: InsightSynthRequest<'_>,
    ) -> Result<Vec<Insight>, AppError>;
}
