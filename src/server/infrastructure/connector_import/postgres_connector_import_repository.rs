use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::connector_import::repository::ConnectorImportRecord;
use crate::server::domain::connector_import::{
    ConnectorImportReadModel, ConnectorImportRepository, ConnectorImportRepositoryError,
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
                connector_id, document_id, source_ref_key,
                first_imported_at, last_imported_at, latest_sync_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (connector_id, document_id) DO UPDATE SET
                source_ref_key = EXCLUDED.source_ref_key,
                last_imported_at = EXCLUDED.last_imported_at,
                latest_sync_id = COALESCE(EXCLUDED.latest_sync_id, connector_imports.latest_sync_id)
            ",
        )
        .bind(record.connector_id)
        .bind(record.document_id)
        .bind(&record.source_ref_key)
        .bind(&record.first_imported_at)
        .bind(&record.last_imported_at)
        .bind(record.latest_sync_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorImportRepositoryError::Internal(format!("upsert: {e}")))?;

        Ok(())
    }

    async fn list_for_documents(
        &self,
        document_ids: &[Uuid],
    ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
        if document_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<ImportRow> = sqlx::query_as(
            "
            SELECT connector_id, document_id, source_ref_key,
                   first_imported_at, last_imported_at, latest_sync_id
            FROM connector_imports
            WHERE document_id = ANY($1)
            ",
        )
        .bind(document_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            ConnectorImportRepositoryError::Internal(format!("list_for_documents: {e}"))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
        let rows: Vec<ImportRow> = sqlx::query_as(
            "
            SELECT connector_id, document_id, source_ref_key,
                   first_imported_at, last_imported_at, latest_sync_id
            FROM connector_imports
            WHERE connector_id = $1
            ORDER BY last_imported_at DESC
            ",
        )
        .bind(connector_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            ConnectorImportRepositoryError::Internal(format!("list_for_connector: {e}"))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn document_ids_for_connectors(
        &self,
        connector_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, ConnectorImportRepositoryError> {
        if connector_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "
            SELECT DISTINCT document_id
            FROM connector_imports
            WHERE connector_id = ANY($1)
            ",
        )
        .bind(connector_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            ConnectorImportRepositoryError::Internal(format!("document_ids_for_connectors: {e}"))
        })?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn facets(&self) -> Result<Vec<(Uuid, u64)>, ConnectorImportRepositoryError> {
        let rows: Vec<(Uuid, i64)> = sqlx::query_as(
            "
            SELECT connector_id, COUNT(DISTINCT document_id)
            FROM connector_imports
            GROUP BY connector_id
            ORDER BY COUNT(DISTINCT document_id) DESC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConnectorImportRepositoryError::Internal(format!("facets: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|(id, count)| (id, count.max(0) as u64))
            .collect())
    }

    async fn find(
        &self,
        connector_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<ConnectorImportReadModel>, ConnectorImportRepositoryError> {
        let row: Option<ImportRow> = sqlx::query_as(
            "
            SELECT connector_id, document_id, source_ref_key,
                   first_imported_at, last_imported_at, latest_sync_id
            FROM connector_imports
            WHERE connector_id = $1 AND document_id = $2
            ",
        )
        .bind(connector_id)
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConnectorImportRepositoryError::Internal(format!("find: {e}")))?;

        Ok(row.map(Into::into))
    }
}

#[derive(sqlx::FromRow)]
struct ImportRow {
    connector_id: Uuid,
    document_id: Uuid,
    source_ref_key: String,
    first_imported_at: String,
    last_imported_at: String,
    latest_sync_id: Option<Uuid>,
}

impl From<ImportRow> for ConnectorImportReadModel {
    fn from(row: ImportRow) -> Self {
        Self {
            connector_id: row.connector_id,
            document_id: row.document_id,
            source_ref_key: row.source_ref_key,
            first_imported_at: row.first_imported_at,
            last_imported_at: row.last_imported_at,
            latest_sync_id: row.latest_sync_id,
        }
    }
}
