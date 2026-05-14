use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::server::application::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointStatus {
    Healthy,
    Error,
}

impl CheckpointStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "error" => Self::Error,
            _ => Self::Healthy,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionCheckpoint {
    pub projector_name: String,
    pub last_processed_log_position: i64,
    pub status: CheckpointStatus,
    pub error_message: Option<String>,
    pub error_count: i64,
    pub updated_at: OffsetDateTime,
}

impl ProjectionCheckpoint {
    pub fn is_faulted(&self, max_errors: i64) -> bool {
        self.status == CheckpointStatus::Error && self.error_count >= max_errors
    }
}

#[async_trait]
pub trait CheckpointRepository: Send + Sync {
    async fn load(&self, projector_name: &str) -> Result<Option<ProjectionCheckpoint>, AppError>;

    async fn upsert(&self, checkpoint: &ProjectionCheckpoint) -> Result<(), AppError>;
}
