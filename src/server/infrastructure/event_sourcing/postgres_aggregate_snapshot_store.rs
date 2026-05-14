use std::marker::PhantomData;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::event_sourcing::aggregate::Aggregate;
use crate::server::event_sourcing::aggregate_repository::{AggregateSnapshot, SnapshotStore};

pub struct PostgresAggregateSnapshotStore<A>
where
    A: Aggregate,
{
    pool: PgPool,
    _phantom: PhantomData<A>,
}

impl<A> PostgresAggregateSnapshotStore<A>
where
    A: Aggregate,
{
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<A> SnapshotStore<A> for PostgresAggregateSnapshotStore<A>
where
    A: Aggregate + 'static,
{
    async fn load(&self, stream_id: Uuid) -> Result<Option<AggregateSnapshot<A>>, AppError> {
        let row: Option<(i64, serde_json::Value)> = sqlx::query_as(
            "SELECT version, snapshot FROM aggregate_snapshots \
             WHERE stream_id = $1 AND aggregate_type = $2",
        )
        .bind(stream_id)
        .bind(A::aggregate_type())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load snapshot: {e}")))?;

        let Some((version, snapshot)) = row else {
            return Ok(None);
        };

        let aggregate: A = serde_json::from_value(snapshot)
            .map_err(|e| AppError::Internal(format!("deserialize snapshot: {e}")))?;
        Ok(Some(AggregateSnapshot {
            stream_id,
            version,
            aggregate,
        }))
    }

    async fn save(&self, snapshot: &AggregateSnapshot<A>) -> Result<(), AppError> {
        let snapshot_json = serde_json::to_value(&snapshot.aggregate)
            .map_err(|e| AppError::Internal(format!("serialize snapshot: {e}")))?;

        sqlx::query(
            "INSERT INTO aggregate_snapshots (stream_id, aggregate_type, version, snapshot, updated_at) \
             VALUES ($1, $2, $3, $4, NOW()) \
             ON CONFLICT (stream_id) DO UPDATE SET \
                 aggregate_type = EXCLUDED.aggregate_type, \
                 version = EXCLUDED.version, \
                 snapshot = EXCLUDED.snapshot, \
                 updated_at = NOW()",
        )
        .bind(snapshot.stream_id)
        .bind(A::aggregate_type())
        .bind(snapshot.version)
        .bind(&snapshot_json)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("save snapshot: {e}")))?;

        Ok(())
    }
}
