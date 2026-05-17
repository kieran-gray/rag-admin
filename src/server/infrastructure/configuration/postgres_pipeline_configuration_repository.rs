use async_trait::async_trait;
use sqlx::{postgres::PgRow, PgPool};
use uuid::Uuid;

use crate::server::domain::configuration::pipeline_configuration::{
    NewPipelineConfiguration, PipelineConfigurationReadModel, PipelineConfigurationRepository,
    PipelineConfigurationRepositoryError, PipelineConfigurationUpdate,
};

pub struct PostgresPipelineConfigurationRepository {
    pool: PgPool,
}

impl PostgresPipelineConfigurationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineConfigurationRepository for PostgresPipelineConfigurationRepository {
    async fn load_all(
        &self,
    ) -> Result<Vec<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError> {
        let rows: Vec<PipelineConfigurationRow> = sqlx::query_as(
            "SELECT id, name, embedding_model_id, generation_model_id, vector_index_id, is_default
             FROM pipeline_configurations
             ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("load_all: {e}")))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError> {
        let row: Option<PipelineConfigurationRow> = sqlx::query_as(
            "SELECT id, name, embedding_model_id, generation_model_id, vector_index_id, is_default
             FROM pipeline_configurations
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }

    async fn find_default(
        &self,
    ) -> Result<Option<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError> {
        let row: Option<PipelineConfigurationRow> = sqlx::query_as(
            "SELECT id, name, embedding_model_id, generation_model_id, vector_index_id, is_default
             FROM pipeline_configurations
             WHERE is_default = TRUE
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            PipelineConfigurationRepositoryError::Internal(format!("find_default: {e}"))
        })?;
        Ok(row.map(Into::into))
    }

    async fn create(
        &self,
        row: NewPipelineConfiguration,
    ) -> Result<(), PipelineConfigurationRepositoryError> {
        sqlx::query(
            "
            INSERT INTO pipeline_configurations (
                id, name, embedding_model_id, generation_model_id, vector_index_id, dimensions
            )
            VALUES (
                $1, $2, $3, $4, $5,
                (SELECT dimensions FROM embedding_models WHERE id = $3)
            )
            ",
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(row.embedding_model_id)
        .bind(row.generation_model_id)
        .bind(row.vector_index_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| map_db_error(&e))
    }

    async fn update(
        &self,
        row: PipelineConfigurationUpdate,
    ) -> Result<(), PipelineConfigurationRepositoryError> {
        let affected = sqlx::query(
            "
            UPDATE pipeline_configurations
            SET name                = $2,
                embedding_model_id  = $3,
                generation_model_id = $4,
                vector_index_id     = $5,
                dimensions          = (SELECT dimensions FROM embedding_models WHERE id = $3),
                updated_at          = NOW()
            WHERE id = $1
            ",
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(row.embedding_model_id)
        .bind(row.generation_model_id)
        .bind(row.vector_index_id)
        .execute(&self.pool)
        .await
        .map_err(|e| map_db_error(&e))?;
        if affected.rows_affected() == 0 {
            return Err(PipelineConfigurationRepositoryError::NotFound(row.id));
        }
        Ok(())
    }

    async fn set_default(&self, id: Uuid) -> Result<(), PipelineConfigurationRepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("tx: {e}")))?;

        sqlx::query("UPDATE pipeline_configurations SET is_default = FALSE WHERE is_default")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                PipelineConfigurationRepositoryError::Internal(format!("clear default: {e}"))
            })?;

        let affected =
            sqlx::query("UPDATE pipeline_configurations SET is_default = TRUE WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    PipelineConfigurationRepositoryError::Internal(format!("set default: {e}"))
                })?;

        if affected.rows_affected() == 0 {
            return Err(PipelineConfigurationRepositoryError::NotFound(id));
        }

        tx.commit()
            .await
            .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("commit: {e}")))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), PipelineConfigurationRepositoryError> {
        let affected = sqlx::query("DELETE FROM pipeline_configurations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("delete: {e}")))?;
        if affected.rows_affected() == 0 {
            return Err(PipelineConfigurationRepositoryError::NotFound(id));
        }
        Ok(())
    }
}

fn map_db_error(error: &sqlx::Error) -> PipelineConfigurationRepositoryError {
    match error {
        sqlx::Error::Database(db) => {
            let code = db
                .code()
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_default();
            match code.as_str() {
                "23505" => PipelineConfigurationRepositoryError::NameConflict,
                "23503" | "23502" => PipelineConfigurationRepositoryError::ReferenceViolation(
                    db.message().to_string(),
                ),
                _ => PipelineConfigurationRepositoryError::Internal(format!(
                    "pipeline configuration: {error}"
                )),
            }
        }
        _ => PipelineConfigurationRepositoryError::Internal(format!(
            "pipeline configuration: {error}"
        )),
    }
}

struct PipelineConfigurationRow {
    id: Uuid,
    name: String,
    embedding_model_id: Uuid,
    generation_model_id: Uuid,
    vector_index_id: Uuid,
    is_default: bool,
}

impl sqlx::FromRow<'_, PgRow> for PipelineConfigurationRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            embedding_model_id: row.try_get("embedding_model_id")?,
            generation_model_id: row.try_get("generation_model_id")?,
            vector_index_id: row.try_get("vector_index_id")?,
            is_default: row.try_get("is_default")?,
        })
    }
}

impl From<PipelineConfigurationRow> for PipelineConfigurationReadModel {
    fn from(row: PipelineConfigurationRow) -> Self {
        Self {
            pipeline_configuration_id: row.id,
            name: row.name,
            embedding_model_id: row.embedding_model_id,
            generation_model_id: row.generation_model_id,
            vector_index_id: row.vector_index_id,
            is_default: row.is_default,
        }
    }
}
