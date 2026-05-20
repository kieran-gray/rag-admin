use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::connector::{
    ConnectorConfig, ConnectorReadModel, ConnectorRepository, ConnectorRepositoryError,
};

pub struct PostgresConnectorRepository {
    pool: PgPool,
}

impl PostgresConnectorRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConnectorRepository for PostgresConnectorRepository {
    async fn load(
        &self,
        connector_id: Uuid,
    ) -> Result<Option<ConnectorReadModel>, ConnectorRepositoryError> {
        let row: Option<ConnectorRow> = sqlx::query_as(
            "
            SELECT connector_id, name, config, deleted, created_at, updated_at,
                   default_pipeline_configuration_id, default_chunking_configuration_id
            FROM connectors
            WHERE connector_id = $1
            ",
        )
        .bind(connector_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConnectorRepositoryError::Internal(format!("load: {e}")))?;

        row.map(ConnectorReadModel::try_from).transpose()
    }

    async fn save(&self, read_model: ConnectorReadModel) -> Result<(), ConnectorRepositoryError> {
        let config = serde_json::to_value(&read_model.config)
            .map_err(|e| ConnectorRepositoryError::Internal(format!("serialize config: {e}")))?;

        sqlx::query(
            "
            INSERT INTO connectors (
                connector_id, name, kind, config, deleted, created_at, updated_at,
                default_pipeline_configuration_id, default_chunking_configuration_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (connector_id) DO UPDATE SET
                name = EXCLUDED.name,
                kind = EXCLUDED.kind,
                config = EXCLUDED.config,
                deleted = EXCLUDED.deleted,
                updated_at = EXCLUDED.updated_at,
                default_pipeline_configuration_id = EXCLUDED.default_pipeline_configuration_id,
                default_chunking_configuration_id = EXCLUDED.default_chunking_configuration_id
            ",
        )
        .bind(read_model.connector_id)
        .bind(&read_model.name)
        .bind(read_model.config.kind().as_str())
        .bind(&config)
        .bind(read_model.deleted)
        .bind(&read_model.created_at)
        .bind(&read_model.updated_at)
        .bind(read_model.default_pipeline_configuration_id)
        .bind(read_model.default_chunking_configuration_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorRepositoryError::Internal(format!("save: {e}")))?;

        Ok(())
    }

    async fn mark_deleted(
        &self,
        connector_id: Uuid,
        updated_at: String,
    ) -> Result<(), ConnectorRepositoryError> {
        sqlx::query(
            "
            UPDATE connectors
            SET deleted = TRUE, updated_at = $2
            WHERE connector_id = $1
            ",
        )
        .bind(connector_id)
        .bind(&updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectorRepositoryError::Internal(format!("mark_deleted: {e}")))?;

        Ok(())
    }

    async fn list_active(&self) -> Result<Vec<ConnectorReadModel>, ConnectorRepositoryError> {
        let rows: Vec<ConnectorRow> = sqlx::query_as(
            "
            SELECT connector_id, name, config, deleted, created_at, updated_at,
                   default_pipeline_configuration_id, default_chunking_configuration_id
            FROM connectors
            WHERE NOT deleted
            ORDER BY created_at ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConnectorRepositoryError::Internal(format!("list_active: {e}")))?;

        rows.into_iter().map(ConnectorReadModel::try_from).collect()
    }
}

#[derive(sqlx::FromRow)]
struct ConnectorRow {
    connector_id: Uuid,
    name: String,
    config: serde_json::Value,
    deleted: bool,
    created_at: String,
    updated_at: String,
    default_pipeline_configuration_id: Option<Uuid>,
    default_chunking_configuration_id: Option<Uuid>,
}

impl TryFrom<ConnectorRow> for ConnectorReadModel {
    type Error = ConnectorRepositoryError;

    fn try_from(row: ConnectorRow) -> Result<Self, Self::Error> {
        let config: ConnectorConfig = serde_json::from_value(row.config)
            .map_err(|e| ConnectorRepositoryError::Internal(format!("deserialize config: {e}")))?;
        Ok(Self {
            connector_id: row.connector_id,
            name: row.name,
            config,
            deleted: row.deleted,
            created_at: row.created_at,
            updated_at: row.updated_at,
            default_pipeline_configuration_id: row.default_pipeline_configuration_id,
            default_chunking_configuration_id: row.default_chunking_configuration_id,
        })
    }
}
