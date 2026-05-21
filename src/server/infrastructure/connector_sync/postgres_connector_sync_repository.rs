use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::connector_sync::repository::ConnectorSyncRecord;
use crate::server::domain::connector_sync::{
    ConnectorSyncRepository, ConnectorSyncRepositoryError, ConnectorSyncSummary,
};

pub struct PostgresConnectorSyncRepository {
    pool: PgPool,
}

impl PostgresConnectorSyncRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConnectorSyncRepository for PostgresConnectorSyncRepository {
    async fn upsert(
        &self,
        record: ConnectorSyncRecord,
    ) -> Result<(), ConnectorSyncRepositoryError> {
        sqlx::query(
            "
            INSERT INTO connector_syncs (
                sync_id, connector_id, status, discovered_count,
                started_at, completed_at, error
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (sync_id) DO UPDATE SET
                status = EXCLUDED.status,
                discovered_count = EXCLUDED.discovered_count,
                completed_at = EXCLUDED.completed_at,
                error = EXCLUDED.error
            ",
        )
        .bind(record.sync_id)
        .bind(record.connector_id)
        .bind(&record.status)
        .bind(record.discovered_count as i32)
        .bind(&record.started_at)
        .bind(&record.completed_at)
        .bind(&record.error)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorSyncRepositoryError::Internal(format!("upsert: {e}")))?;

        Ok(())
    }

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
        let rows: Vec<SyncRow> = sqlx::query_as(
            "
            SELECT sync_id, connector_id, status, discovered_count,
                   started_at, completed_at, error
            FROM connector_syncs
            WHERE connector_id = $1
            ORDER BY started_at DESC
            LIMIT $2 OFFSET $3
            ",
        )
        .bind(connector_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConnectorSyncRepositoryError::Internal(format!("list_for_connector: {e}")))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn latest_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
        let row: Option<SyncRow> = sqlx::query_as(
            "
            SELECT sync_id, connector_id, status, discovered_count,
                   started_at, completed_at, error
            FROM connector_syncs
            WHERE connector_id = $1
            ORDER BY started_at DESC
            LIMIT 1
            ",
        )
        .bind(connector_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            ConnectorSyncRepositoryError::Internal(format!("latest_for_connector: {e}"))
        })?;

        Ok(row.map(Into::into))
    }

    async fn find_by_sync_id(
        &self,
        sync_id: Uuid,
    ) -> Result<Option<ConnectorSyncSummary>, ConnectorSyncRepositoryError> {
        let row: Option<SyncRow> = sqlx::query_as(
            "
            SELECT sync_id, connector_id, status, discovered_count,
                   started_at, completed_at, error
            FROM connector_syncs
            WHERE sync_id = $1
            ",
        )
        .bind(sync_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConnectorSyncRepositoryError::Internal(format!("find_by_sync_id: {e}")))?;

        Ok(row.map(Into::into))
    }
}

#[derive(sqlx::FromRow)]
struct SyncRow {
    sync_id: Uuid,
    connector_id: Uuid,
    status: String,
    discovered_count: i32,
    started_at: String,
    completed_at: Option<String>,
    error: Option<String>,
}

impl From<SyncRow> for ConnectorSyncSummary {
    fn from(row: SyncRow) -> Self {
        Self {
            sync_id: row.sync_id,
            connector_id: row.connector_id,
            status: row.status,
            discovered_count: row.discovered_count.max(0) as u32,
            started_at: row.started_at,
            completed_at: row.completed_at,
            error: row.error,
        }
    }
}
