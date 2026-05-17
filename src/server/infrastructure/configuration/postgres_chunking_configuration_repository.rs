use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::ChunkingConfig;
use crate::server::domain::configuration::chunking_configuration::{
    ChunkingConfigurationReadModel, ChunkingConfigurationRepository,
    ChunkingConfigurationRepositoryError, ChunkingConfigurationUpdate, NewChunkingConfiguration,
};

pub struct PostgresChunkingConfigurationRepository {
    pool: PgPool,
}

impl PostgresChunkingConfigurationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChunkingConfigurationRepository for PostgresChunkingConfigurationRepository {
    async fn load_all(
        &self,
    ) -> Result<Vec<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError> {
        let rows: Vec<ChunkingConfigurationRow> = sqlx::query_as(
            "SELECT id, name, config, is_default
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
        let row: Option<ChunkingConfigurationRow> = sqlx::query_as(
            "SELECT id, name, config, is_default FROM chunking_configurations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChunkingConfigurationRepositoryError::Internal(format!("find_by_id: {e}")))?;
        row.map(TryInto::try_into).transpose()
    }

    async fn find_default(
        &self,
    ) -> Result<Option<ChunkingConfigurationReadModel>, ChunkingConfigurationRepositoryError> {
        let row: Option<ChunkingConfigurationRow> = sqlx::query_as(
            "SELECT id, name, config, is_default
             FROM chunking_configurations
             WHERE is_default = TRUE
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            ChunkingConfigurationRepositoryError::Internal(format!("find_default: {e}"))
        })?;
        row.map(TryInto::try_into).transpose()
    }

    async fn create(
        &self,
        row: NewChunkingConfiguration,
    ) -> Result<(), ChunkingConfigurationRepositoryError> {
        let config_json = serialize_config(&row.config)?;
        let generation_model_id = generation_model_id(&row.config);
        sqlx::query(
            "
            INSERT INTO chunking_configurations (id, name, generation_model_id, config)
            VALUES ($1, $2, $3, $4)
            ",
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(generation_model_id)
        .bind(&config_json)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| map_db_error(&e))
    }

    async fn update(
        &self,
        row: ChunkingConfigurationUpdate,
    ) -> Result<(), ChunkingConfigurationRepositoryError> {
        let config_json = serialize_config(&row.config)?;
        let generation_model_id = generation_model_id(&row.config);
        let affected = sqlx::query(
            "
            UPDATE chunking_configurations
            SET name                = $2,
                generation_model_id = $3,
                config              = $4,
                updated_at          = NOW()
            WHERE id = $1
            ",
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(generation_model_id)
        .bind(&config_json)
        .execute(&self.pool)
        .await
        .map_err(|e| map_db_error(&e))?;
        if affected.rows_affected() == 0 {
            return Err(ChunkingConfigurationRepositoryError::NotFound(row.id));
        }
        Ok(())
    }

    async fn set_default(&self, id: Uuid) -> Result<(), ChunkingConfigurationRepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ChunkingConfigurationRepositoryError::Internal(format!("tx: {e}")))?;

        sqlx::query("UPDATE chunking_configurations SET is_default = FALSE WHERE is_default")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                ChunkingConfigurationRepositoryError::Internal(format!("clear default: {e}"))
            })?;

        let affected =
            sqlx::query("UPDATE chunking_configurations SET is_default = TRUE WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    ChunkingConfigurationRepositoryError::Internal(format!("set default: {e}"))
                })?;

        if affected.rows_affected() == 0 {
            return Err(ChunkingConfigurationRepositoryError::NotFound(id));
        }

        tx.commit()
            .await
            .map_err(|e| ChunkingConfigurationRepositoryError::Internal(format!("commit: {e}")))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), ChunkingConfigurationRepositoryError> {
        let affected = sqlx::query("DELETE FROM chunking_configurations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| ChunkingConfigurationRepositoryError::Internal(format!("delete: {e}")))?;
        if affected.rows_affected() == 0 {
            return Err(ChunkingConfigurationRepositoryError::NotFound(id));
        }
        Ok(())
    }
}

fn serialize_config(
    config: &ChunkingConfig,
) -> Result<serde_json::Value, ChunkingConfigurationRepositoryError> {
    serde_json::to_value(config).map_err(|e| {
        ChunkingConfigurationRepositoryError::Internal(format!("serialize config: {e}"))
    })
}

fn generation_model_id(config: &ChunkingConfig) -> Option<Uuid> {
    match config {
        ChunkingConfig::Llm(llm) => Some(llm.generation_model_id),
        _ => None,
    }
}

fn map_db_error(error: &sqlx::Error) -> ChunkingConfigurationRepositoryError {
    match error {
        sqlx::Error::Database(db) => {
            let code = db
                .code()
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_default();
            match code.as_str() {
                "23505" => ChunkingConfigurationRepositoryError::NameConflict,
                "23503" => ChunkingConfigurationRepositoryError::ReferenceViolation(
                    db.message().to_string(),
                ),
                _ => ChunkingConfigurationRepositoryError::Internal(format!(
                    "chunking configuration: {error}"
                )),
            }
        }
        _ => ChunkingConfigurationRepositoryError::Internal(format!(
            "chunking configuration: {error}"
        )),
    }
}

struct ChunkingConfigurationRow {
    id: Uuid,
    name: String,
    config: serde_json::Value,
    is_default: bool,
}

impl sqlx::FromRow<'_, PgRow> for ChunkingConfigurationRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            config: row.try_get("config")?,
            is_default: row.try_get("is_default")?,
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
            is_default: row.is_default,
        })
    }
}
