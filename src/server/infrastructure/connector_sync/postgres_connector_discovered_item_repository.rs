use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::connector_sync::{
    ConnectorDiscoveredItemReadModel, ConnectorDiscoveredItemRepository,
    ConnectorDiscoveredItemRepositoryError, DiscoveredItemUpsert,
};

pub struct PostgresConnectorDiscoveredItemRepository {
    pool: PgPool,
}

impl PostgresConnectorDiscoveredItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConnectorDiscoveredItemRepository for PostgresConnectorDiscoveredItemRepository {
    async fn upsert(
        &self,
        item: DiscoveredItemUpsert,
    ) -> Result<(), ConnectorDiscoveredItemRepositoryError> {
        sqlx::query(
            "
            INSERT INTO connector_discovered_items (
                connector_id, source_ref_key, title,
                first_seen, last_seen, latest_sync_id
            )
            VALUES ($1, $2, $3, $4, $4, $5)
            ON CONFLICT (connector_id, source_ref_key) DO UPDATE SET
                title = EXCLUDED.title,
                last_seen = EXCLUDED.last_seen,
                latest_sync_id = EXCLUDED.latest_sync_id
            ",
        )
        .bind(item.connector_id)
        .bind(&item.source_ref_key)
        .bind(&item.title)
        .bind(&item.seen_at)
        .bind(item.sync_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorDiscoveredItemRepositoryError::Internal(format!("upsert: {e}")))?;

        Ok(())
    }

    async fn list_for_connector(
        &self,
        connector_id: Uuid,
    ) -> Result<Vec<ConnectorDiscoveredItemReadModel>, ConnectorDiscoveredItemRepositoryError> {
        let rows: Vec<ItemRow> = sqlx::query_as(
            "
            SELECT connector_id, source_ref_key, title,
                   first_seen, last_seen, latest_sync_id
            FROM connector_discovered_items
            WHERE connector_id = $1
            ORDER BY last_seen DESC
            ",
        )
        .bind(connector_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            ConnectorDiscoveredItemRepositoryError::Internal(format!("list_for_connector: {e}"))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    connector_id: Uuid,
    source_ref_key: String,
    title: String,
    first_seen: String,
    last_seen: String,
    latest_sync_id: Uuid,
}

impl From<ItemRow> for ConnectorDiscoveredItemReadModel {
    fn from(row: ItemRow) -> Self {
        Self {
            connector_id: row.connector_id,
            source_ref_key: row.source_ref_key,
            title: row.title,
            first_seen: row.first_seen,
            last_seen: row.last_seen,
            latest_sync_id: row.latest_sync_id,
        }
    }
}
