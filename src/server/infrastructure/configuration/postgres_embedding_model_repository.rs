use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::embedding_model::{
    EmbeddingModel, EmbeddingModelRepository, EmbeddingModelRepositoryError,
};
use crate::server::domain::configuration::kinds::AiProviderKind;

pub struct PostgresEmbeddingModelRepository {
    pool: PgPool,
}

impl PostgresEmbeddingModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EmbeddingModelRepository for PostgresEmbeddingModelRepository {
    async fn load_all(&self) -> Result<Vec<EmbeddingModel>, EmbeddingModelRepositoryError> {
        let rows: Vec<EmbeddingModelRow> = sqlx::query_as(
            r#"
            SELECT id, kind, model, dimensions
            FROM embedding_models
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EmbeddingModelRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<EmbeddingModel>, EmbeddingModelRepositoryError> {
        let row: Option<EmbeddingModelRow> = sqlx::query_as(
            "SELECT id, kind, model, dimensions FROM embedding_models WHERE id = $1",
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EmbeddingModelRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }

    async fn save(&self, model: EmbeddingModel) -> Result<(), EmbeddingModelRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO embedding_models (id, kind, model, dimensions)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                kind       = $2,
                model      = $3,
                dimensions = $4,
                updated_at = NOW()
            "#,
        )
        .bind(model.embedding_model_id)
        .bind(model.kind.as_str())
        .bind(&model.model)
        .bind(model.dimensions as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| EmbeddingModelRepositoryError::Internal(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, model_id: Uuid) -> Result<(), EmbeddingModelRepositoryError> {
        sqlx::query("DELETE FROM embedding_models WHERE id = $1")
            .bind(model_id)
            .execute(&self.pool)
            .await
            .map_err(|e| EmbeddingModelRepositoryError::Internal(format!("delete: {e}")))?;
        Ok(())
    }
}

struct EmbeddingModelRow {
    id: Uuid,
    kind: String,
    model: String,
    dimensions: i32,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for EmbeddingModelRow {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            kind: row.try_get("kind")?,
            model: row.try_get("model")?,
            dimensions: row.try_get("dimensions")?,
        })
    }
}

impl From<EmbeddingModelRow> for EmbeddingModel {
    fn from(row: EmbeddingModelRow) -> Self {
        Self {
            embedding_model_id: row.id,
            kind: AiProviderKind::parse(&row.kind)
                .expect("unknown ai provider kind in embedding_models"),
            model: row.model,
            dimensions: row.dimensions as u32,
        }
    }
}
