use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::thread::Thread;

pub struct ThreadSynthRequest<'a> {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub section_sequence: u32,
    pub observations: &'a [Observation],
    pub carried_summary: &'a str,
}

#[derive(Debug, Clone)]
pub struct ThreadSynthResult {
    pub threads: Vec<Thread>,
    pub updated_carried_summary: String,
}

#[async_trait]
pub trait ThreadSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        generation_model_id: Uuid,
        req: ThreadSynthRequest<'_>,
    ) -> Result<ThreadSynthResult, AppError>;
}
