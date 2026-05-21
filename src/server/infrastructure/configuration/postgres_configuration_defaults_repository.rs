use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::defaults::{
    ConfigurationDefaultsReadModel, ConfigurationDefaultsRepository,
    ConfigurationDefaultsRepositoryError,
};
use event_sourcing::error::ProjectionError;

pub struct PostgresConfigurationDefaultsRepository {
    pool: PgPool,
}

impl PostgresConfigurationDefaultsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConfigurationDefaultsRepository for PostgresConfigurationDefaultsRepository {
    async fn load(
        &self,
    ) -> Result<ConfigurationDefaultsReadModel, ConfigurationDefaultsRepositoryError> {
        let row: Option<ConfigurationDefaultsRow> = sqlx::query_as(
            "SELECT chunking_configuration_id, index_profile_id, retrieval_profile_id, sweep_template_id
             FROM configuration_defaults
             WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigurationDefaultsRepositoryError::Internal(format!("load: {e}")))?;
        Ok(row.map(Into::into).unwrap_or_default())
    }

    async fn set_chunking_configuration(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query(
            "INSERT INTO configuration_defaults (id, chunking_configuration_id)
             VALUES (1, $1)
             ON CONFLICT (id) DO UPDATE SET chunking_configuration_id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| ProjectionError::Storage(format!("set default chunking: {e}")))
    }

    async fn set_index_profile(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query(
            "INSERT INTO configuration_defaults (id, index_profile_id)
             VALUES (1, $1)
             ON CONFLICT (id) DO UPDATE SET index_profile_id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| ProjectionError::Storage(format!("set default index profile: {e}")))
    }

    async fn set_retrieval_profile(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query(
            "INSERT INTO configuration_defaults (id, retrieval_profile_id)
             VALUES (1, $1)
             ON CONFLICT (id) DO UPDATE SET retrieval_profile_id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| ProjectionError::Storage(format!("set default retrieval profile: {e}")))
    }
}

struct ConfigurationDefaultsRow {
    chunking_configuration_id: Option<Uuid>,
    index_profile_id: Option<Uuid>,
    retrieval_profile_id: Option<Uuid>,
    sweep_template_id: Option<Uuid>,
}

impl sqlx::FromRow<'_, PgRow> for ConfigurationDefaultsRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            chunking_configuration_id: row.try_get("chunking_configuration_id")?,
            index_profile_id: row.try_get("index_profile_id")?,
            retrieval_profile_id: row.try_get("retrieval_profile_id")?,
            sweep_template_id: row.try_get("sweep_template_id")?,
        })
    }
}

impl From<ConfigurationDefaultsRow> for ConfigurationDefaultsReadModel {
    fn from(row: ConfigurationDefaultsRow) -> Self {
        Self {
            chunking_configuration_id: row.chunking_configuration_id,
            index_profile_id: row.index_profile_id,
            retrieval_profile_id: row.retrieval_profile_id,
            sweep_template_id: row.sweep_template_id,
        }
    }
}
