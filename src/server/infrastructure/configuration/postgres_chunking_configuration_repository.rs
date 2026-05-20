use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::chunking_configuration::{
    ChunkingConfiguration, ChunkingConfigurationReadModel, ChunkingConfigurationRepository,
    ChunkingConfigurationRepositoryError,
};
use crate::server::domain::shared::value_objects::ChunkingConfig;
use event_sourcing::error::ProjectionError;

pub struct PostgresChunkingConfigurationRepository {
    pool: PgPool,
}

impl PostgresChunkingConfigurationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<ChunkingConfiguration> for PostgresChunkingConfigurationRepository {
    async fn upsert(&self, entry: ChunkingConfiguration) -> Result<(), ProjectionError> {
        let generation_model_id = generation_model_id(&entry.config);
        let config_json = serde_json::to_value(entry.config)
            .map_err(|e| ProjectionError::Storage(format!("serialize config: {e}")))?;
        sqlx::query(
            "
            INSERT INTO chunking_configurations (id, name, generation_model_id, config)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                name                = $2,
                generation_model_id = $3,
                config              = $4,
                updated_at          = NOW()
            ",
        )
        .bind(entry.chunking_configuration_id)
        .bind(&entry.name)
        .bind(generation_model_id)
        .bind(&config_json)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| ProjectionError::Storage(format!("upsert chunking config: {e}")))
    }

    async fn delete(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM chunking_configurations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| ProjectionError::Storage(format!("delete chunking config: {e}")))
    }
}

#[async_trait]
impl ChunkingConfigurationRepository for PostgresChunkingConfigurationRepository {
    async fn load_all(
        &self,
    ) -> Result<Vec<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError> {
        let rows: Vec<ChunkingConfigurationRow> = sqlx::query_as(
            "SELECT id, name, config
             FROM chunking_configurations
             ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkingConfigurationRepositoryError::Internal(format!("load_all: {e}")))?;

        rows.into_iter()
            .map(ChunkingConfigurationRow::try_into)
            .collect()
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError> {
        let row: Option<ChunkingConfigurationRow> =
            sqlx::query_as("SELECT id, name, config FROM chunking_configurations WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    ChunkingConfigurationRepositoryError::Internal(format!("find_by_id: {e}"))
                })?;
        row.map(TryInto::try_into).transpose()
    }
}

fn generation_model_id(config: &ChunkingConfig) -> Option<Uuid> {
    match config {
        ChunkingConfig::Llm(llm) => Some(llm.generation_model_id),
        _ => None,
    }
}

struct ChunkingConfigurationRow {
    id: Uuid,
    name: String,
    config: serde_json::Value,
}

impl sqlx::FromRow<'_, PgRow> for ChunkingConfigurationRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            config: row.try_get("config")?,
        })
    }
}

impl TryFrom<ChunkingConfigurationRow> for ChunkingConfigurationReadModel {
    type Error = ChunkingConfigurationRepositoryError;

    fn try_from(row: ChunkingConfigurationRow) -> Result<Self, Self::Error> {
        let config: ChunkingConfig = serde_json::from_value(row.config).map_err(|e| {
            ChunkingConfigurationRepositoryError::Internal(format!("deserialize config: {e}"))
        })?;
        Ok(Self {
            chunking_configuration_id: row.id,
            name: row.name,
            config,
        })
    }
}
