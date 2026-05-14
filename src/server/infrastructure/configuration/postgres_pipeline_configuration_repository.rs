use async_trait::async_trait;
use sqlx::PgPool;
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
            r#"
            SELECT id, name, embedding_model_id, generation_model_id, vector_index_id
            FROM pipeline_configurations
            ORDER BY created_at ASC
            "#,
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
            r#"
            SELECT id, name, embedding_model_id, generation_model_id, vector_index_id
            FROM pipeline_configurations
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }

    async fn create(
        &self,
        row: NewPipelineConfiguration,
    ) -> Result<(), PipelineConfigurationRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO pipeline_configurations (
                id, name, embedding_model_id, generation_model_id, vector_index_id, dimensions
            )
            VALUES (
                $1, $2, $3, $4, $5,
                (SELECT dimensions FROM embedding_models WHERE id = $3)
            )
            "#,
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(row.embedding_model_id)
        .bind(row.generation_model_id)
        .bind(row.vector_index_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(map_db_error)
    }

    async fn update(
        &self,
        row: PipelineConfigurationUpdate,
    ) -> Result<(), PipelineConfigurationRepositoryError> {
        let affected = sqlx::query(
            r#"
            UPDATE pipeline_configurations
            SET name                = $2,
                embedding_model_id  = $3,
                generation_model_id = $4,
                vector_index_id     = $5,
                dimensions          = (SELECT dimensions FROM embedding_models WHERE id = $3),
                updated_at          = NOW()
            WHERE id = $1
            "#,
        )
        .bind(row.id)
        .bind(&row.name)
        .bind(row.embedding_model_id)
        .bind(row.generation_model_id)
        .bind(row.vector_index_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        if affected.rows_affected() == 0 {
            return Err(PipelineConfigurationRepositoryError::NotFound(row.id));
        }
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

fn map_db_error(error: sqlx::Error) -> PipelineConfigurationRepositoryError {
    match &error {
        sqlx::Error::Database(db) => {
            let code = db.code().map(|c| c.into_owned()).unwrap_or_default();
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
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for PipelineConfigurationRow {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            embedding_model_id: row.try_get("embedding_model_id")?,
            generation_model_id: row.try_get("generation_model_id")?,
            vector_index_id: row.try_get("vector_index_id")?,
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
        }
    }
}
