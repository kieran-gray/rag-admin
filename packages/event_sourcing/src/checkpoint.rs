use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::error::EsError;

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
    async fn load(&self, projector_name: &str) -> Result<Option<ProjectionCheckpoint>, EsError>;

    async fn upsert(&self, checkpoint: &ProjectionCheckpoint) -> Result<(), EsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(status: CheckpointStatus, error_count: i64) -> ProjectionCheckpoint {
        ProjectionCheckpoint {
            projector_name: "p".into(),
            last_processed_log_position: 0,
            status,
            error_message: None,
            error_count,
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn test_as_str_conversions() {
        assert_eq!(CheckpointStatus::Error.as_str(), "error");
        assert_eq!(CheckpointStatus::Healthy.as_str(), "healthy");
    }

    #[test]
    fn faulted_at_exactly_max_errors() {
        assert!(cp(CheckpointStatus::Error, 5).is_faulted(5));
    }

    #[test]
    fn not_faulted_below_max_errors() {
        assert!(!cp(CheckpointStatus::Error, 4).is_faulted(5));
    }

    #[test]
    fn healthy_is_never_faulted() {
        assert!(!cp(CheckpointStatus::Healthy, 100).is_faulted(1));
    }

    #[test]
    fn status_round_trips_through_str() {
        assert_eq!(
            CheckpointStatus::parse("healthy"),
            CheckpointStatus::Healthy
        );
        assert_eq!(CheckpointStatus::parse("error"), CheckpointStatus::Error);
        assert_eq!(CheckpointStatus::parse("other"), CheckpointStatus::Healthy);
    }
}
