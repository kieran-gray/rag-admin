use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::EsError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    pub fn new(stream_id: Uuid, event_log_position: i64, discriminator: &str) -> Self {
        Self(format!("{stream_id}:{event_log_position}:{discriminator}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct NewJob<R> {
    pub partition_key: Uuid,
    pub job_type: &'static str,
    pub idempotency_key: IdempotencyKey,
    pub payload: R,
}

#[derive(Debug, Clone)]
pub struct ClaimedJob<R> {
    pub id: Uuid,
    pub partition_key: Option<Uuid>,
    pub job_type: String,
    pub idempotency_key: IdempotencyKey,
    pub attempts: i32,
    pub max_attempts: i32,
    pub payload: R,
}

#[async_trait]
pub trait JobQueue<R>: Send + Sync
where
    R: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    async fn enqueue(&self, queue: &str, jobs: &[NewJob<R>]) -> Result<(), EsError>;

    async fn claim(
        &self,
        queue: &str,
        worker_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedJob<R>>, EsError>;

    async fn heartbeat(
        &self,
        job_id: Uuid,
        worker_id: &str,
        lease: Duration,
    ) -> Result<bool, EsError>;

    async fn ack(&self, job_id: Uuid, worker_id: &str) -> Result<(), EsError>;

    async fn nack(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
        backoff: Duration,
    ) -> Result<(), EsError>;

    async fn dead_letter(&self, job_id: Uuid, error: &str) -> Result<(), EsError>;

    async fn cancel_partition(&self, queue: &str, partition_key: Uuid) -> Result<u64, EsError>;
}
