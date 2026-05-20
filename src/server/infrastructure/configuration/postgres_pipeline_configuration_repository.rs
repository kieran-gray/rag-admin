use async_trait::async_trait;
use sqlx::{postgres::PgRow, PgPool};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::pipeline_configuration::{
    PipelineConfiguration, PipelineConfigurationReadModel, PipelineConfigurationRepository,
    PipelineConfigurationRepositoryError,
};
use event_sourcing::error::ProjectionError;

pub struct PostgresPipelineConfigurationRepository {
    pool: PgPool,
}

impl PostgresPipelineConfigurationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<PipelineConfiguration> for PostgresPipelineConfigurationRepository {
    async fn upsert(&self, entry: PipelineConfiguration) -> Result<(), ProjectionError> {
        sqlx::query(
            "
            INSERT INTO pipeline_configurations (
                id, name, embedding_model_id, generation_model_id, vector_index_id, dimensions
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                name                = $2,
                embedding_model_id  = $3,
                generation_model_id = $4,
                vector_index_id     = $5,
                dimensions          = $6,
                updated_at          = NOW()
            ",
        )
        .bind(entry.pipeline_configuration_id)
        .bind(&entry.name)
        .bind(entry.embedding_model_id)
        .bind(entry.generation_model_id)
        .bind(entry.vector_index_id)
        .bind(entry.dimensions as i32)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| ProjectionError::Storage(format!("upsert pipeline config: {e}")))
    }

    async fn delete(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM pipeline_configurations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| ProjectionError::Storage(format!("delete pipeline config: {e}")))
    }
}

#[async_trait]
impl PipelineConfigurationRepository for PostgresPipelineConfigurationRepository {
    async fn load_all(
        &self,
    ) -> Result<Vec<PipelineConfigurationReadModel>, PipelineConfigurationRepositoryError> {
        let rows: Vec<PipelineConfigurationRow> = sqlx::query_as(
            "SELECT id, name, embedding_model_id, generation_model_id, vector_index_id, dimensions
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
            "SELECT id, name, embedding_model_id, generation_model_id, vector_index_id, dimensions
             FROM pipeline_configurations
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PipelineConfigurationRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }
}

struct PipelineConfigurationRow {
    id: Uuid,
    name: String,
    embedding_model_id: Uuid,
    generation_model_id: Uuid,
    vector_index_id: Uuid,
    dimensions: i32,
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
            dimensions: row.try_get("dimensions")?,
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
            dimensions: row.dimensions as u32,
        }
    }
}
