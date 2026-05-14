use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::application::AppError;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectStatus {
    Pending,
    Dispatched,
    Completed,
    Failed,
}

impl EffectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "dispatched" => Self::Dispatched,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingEffect<R> {
    pub stream_id: Uuid,
    pub event_log_position: i64,
    pub effect_type: &'static str,
    pub idempotency_key: IdempotencyKey,
    pub payload: R,
}

#[derive(Debug, Clone)]
pub struct EffectRecord<R> {
    pub effect_id: Uuid,
    pub stream_id: Uuid,
    pub event_log_position: i64,
    pub idempotency_key: IdempotencyKey,
    pub status: EffectStatus,
    pub attempts: i32,
    pub payload: R,
}

#[async_trait]
pub trait EffectLedger<R>: Send + Sync
where
    R: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    async fn insert(
        &self,
        aggregate_type: &str,
        effects: &[PendingEffect<R>],
    ) -> Result<(), AppError>;

    async fn pending(
        &self,
        aggregate_type: &str,
        max_attempts: i32,
    ) -> Result<Vec<EffectRecord<R>>, AppError>;

    async fn mark_dispatched(&self, effect_id: Uuid) -> Result<(), AppError>;

    async fn mark_completed(&self, effect_id: Uuid) -> Result<(), AppError>;

    async fn mark_failed(
        &self,
        effect_id: Uuid,
        error: &str,
        attempts: i32,
    ) -> Result<(), AppError>;
}
