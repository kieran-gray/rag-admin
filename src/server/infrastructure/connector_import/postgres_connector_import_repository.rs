use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::connector_import::repository::ConnectorImportRecord;
use crate::server::domain::connector_import::{
    ConnectorImportRepository, ConnectorImportRepositoryError, ConnectorImportSummary,
};

pub struct PostgresConnectorImportRepository {
    pool: PgPool,
}

impl PostgresConnectorImportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConnectorImportRepository for PostgresConnectorImportRepository {
    async fn upsert(
        &self,
        record: ConnectorImportRecord,
    ) -> Result<(), ConnectorImportRepositoryError> {
        sqlx::query(
            "
            INSERT INTO connector_imports (
                import_id, connector_id, status, total, imported, failed,
                index_after_import, started_at, completed_at, error
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (import_id) DO UPDATE SET
                status = EXCLUDED.status,
                total = EXCLUDED.total,
                imported = EXCLUDED.imported,
                failed = EXCLUDED.failed,
                index_after_import = EXCLUDED.index_after_import,
                completed_at = EXCLUDED.completed_at,
                error = EXCLUDED.error
            ",
        )
        .bind(record.import_id)
        .bind(record.connector_id)
        .bind(&record.status)
        .bind(record.total as i32)
        .bind(record.imported as i32)
        .bind(record.failed as i32)
        .bind(record.index_after_import)
        .bind(&record.started_at)
        .bind(&record.completed_at)
        .bind(&record.error)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorImportRepositoryError::Internal(format!("upsert: {e}")))?;

        Ok(())
    }

    async fn find_by_import_id(
        &self,
        import_id: Uuid,
    ) -> Result<Option<ConnectorImportSummary>, ConnectorImportRepositoryError> {
        let row: Option<ImportRow> = sqlx::query_as(
            "
            SELECT import_id, connector_id, status, total, imported, failed,
                   index_after_import, started_at, completed_at, error
            FROM connector_imports
            WHERE import_id = $1
            ",
        )
        .bind(import_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConnectorImportRepositoryError::Internal(format!("find_by_import_id: {e}")))?;

        Ok(row.map(Into::into))
    }

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ConnectorImportSummary>, ConnectorImportRepositoryError> {
        let rows: Vec<ImportRow> = sqlx::query_as(
            "
            SELECT import_id, connector_id, status, total, imported, failed,
                   index_after_import, started_at, completed_at, error
            FROM connector_imports
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
        .map_err(|e| {
            ConnectorImportRepositoryError::Internal(format!("list_for_connector: {e}"))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(sqlx::FromRow)]
struct ImportRow {
    import_id: Uuid,
    connector_id: Uuid,
    status: String,
    total: i32,
    imported: i32,
    failed: i32,
    index_after_import: bool,
    started_at: String,
    completed_at: Option<String>,
    error: Option<String>,
}

impl From<ImportRow> for ConnectorImportSummary {
    fn from(row: ImportRow) -> Self {
        Self {
            import_id: row.import_id,
            connector_id: row.connector_id,
            status: row.status,
            total: row.total.max(0) as u32,
            imported: row.imported.max(0) as u32,
            failed: row.failed.max(0) as u32,
            index_after_import: row.index_after_import,
            started_at: row.started_at,
            completed_at: row.completed_at,
            error: row.error,
        }
    }
}
