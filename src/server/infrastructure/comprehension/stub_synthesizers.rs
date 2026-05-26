use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::comprehension::ports::{
    InsightSynthRequest, InsightSynthesizer, ThreadSynthRequest, ThreadSynthResult,
    ThreadSynthesizer,
};
use crate::server::application::AppError;
use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::thread::Thread;

pub struct NoopThreadSynthesizer;

#[async_trait]
impl ThreadSynthesizer for NoopThreadSynthesizer {
    async fn synthesize(
        &self,
        _generation_model_id: Uuid,
        req: ThreadSynthRequest<'_>,
    ) -> Result<ThreadSynthResult, AppError> {
        Ok(ThreadSynthResult {
            threads: Vec::<Thread>::new(),
            updated_carried_summary: req.carried_summary.to_string(),
        })
    }
}

pub struct NoopInsightSynthesizer;

#[async_trait]
impl InsightSynthesizer for NoopInsightSynthesizer {
    async fn synthesize(
        &self,
        _generation_model_id: Uuid,
        _req: InsightSynthRequest<'_>,
    ) -> Result<Vec<Insight>, AppError> {
        Ok(Vec::new())
    }
}
