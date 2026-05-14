use async_trait::async_trait;
use sqlx::PgPool;

use crate::server::application::AppError;
use crate::server::event_sourcing::checkpoint::{
    CheckpointRepository, CheckpointStatus, ProjectionCheckpoint,
};

pub struct PostgresCheckpointRepository {
    pool: PgPool,
}

impl PostgresCheckpointRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CheckpointRepository for PostgresCheckpointRepository {
    async fn load(&self, projector_name: &str) -> Result<Option<ProjectionCheckpoint>, AppError> {
        let row: Option<CheckpointRow> = sqlx::query_as(
            "SELECT projector_name, last_processed_log_position, status, error_message, error_count, updated_at \
             FROM projection_checkpoints WHERE projector_name = $1",
        )
        .bind(projector_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load checkpoint: {e}")))?;

        Ok(row.map(Into::into))
    }

    async fn upsert(&self, checkpoint: &ProjectionCheckpoint) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO projection_checkpoints \
                 (projector_name, last_processed_log_position, status, error_message, error_count, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (projector_name) DO UPDATE SET \
                 last_processed_log_position = EXCLUDED.last_processed_log_position, \
                 status = EXCLUDED.status, \
                 error_message = EXCLUDED.error_message, \
                 error_count = EXCLUDED.error_count, \
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(&checkpoint.projector_name)
        .bind(checkpoint.last_processed_log_position)
        .bind(checkpoint.status.as_str())
        .bind(&checkpoint.error_message)
        .bind(checkpoint.error_count)
        .bind(checkpoint.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("upsert checkpoint: {e}")))?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct CheckpointRow {
    projector_name: String,
    last_processed_log_position: i64,
    status: String,
    error_message: Option<String>,
    error_count: i64,
    updated_at: time::OffsetDateTime,
}

impl From<CheckpointRow> for ProjectionCheckpoint {
    fn from(row: CheckpointRow) -> Self {
        Self {
            projector_name: row.projector_name,
            last_processed_log_position: row.last_processed_log_position,
            status: CheckpointStatus::parse(&row.status),
            error_message: row.error_message,
            error_count: row.error_count,
            updated_at: row.updated_at,
        }
    }
}
