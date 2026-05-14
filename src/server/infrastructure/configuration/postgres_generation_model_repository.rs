use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::generation_model::{
    GenerationModel, GenerationModelRepository, GenerationModelRepositoryError,
};
use crate::server::domain::configuration::kinds::AiProviderKind;

pub struct PostgresGenerationModelRepository {
    pool: PgPool,
}

impl PostgresGenerationModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GenerationModelRepository for PostgresGenerationModelRepository {
    async fn load_all(&self) -> Result<Vec<GenerationModel>, GenerationModelRepositoryError> {
        let rows: Vec<GenerationModelRow> = sqlx::query_as(
            r#"
            SELECT id, kind, model
            FROM generation_models
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GenerationModelRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<GenerationModel>, GenerationModelRepositoryError> {
        let row: Option<GenerationModelRow> =
            sqlx::query_as("SELECT id, kind, model FROM generation_models WHERE id = $1")
                .bind(model_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    GenerationModelRepositoryError::Internal(format!("find_by_id: {e}"))
                })?;
        Ok(row.map(Into::into))
    }

    async fn save(&self, model: GenerationModel) -> Result<(), GenerationModelRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO generation_models (id, kind, model)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET
                kind       = $2,
                model      = $3,
                updated_at = NOW()
            "#,
        )
        .bind(model.generation_model_id)
        .bind(model.kind.as_str())
        .bind(&model.model)
        .execute(&self.pool)
        .await
        .map_err(|e| GenerationModelRepositoryError::Internal(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, model_id: Uuid) -> Result<(), GenerationModelRepositoryError> {
        sqlx::query("DELETE FROM generation_models WHERE id = $1")
            .bind(model_id)
            .execute(&self.pool)
            .await
            .map_err(|e| GenerationModelRepositoryError::Internal(format!("delete: {e}")))?;
        Ok(())
    }
}

struct GenerationModelRow {
    id: Uuid,
    kind: String,
    model: String,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for GenerationModelRow {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            kind: row.try_get("kind")?,
            model: row.try_get("model")?,
        })
    }
}

impl From<GenerationModelRow> for GenerationModel {
    fn from(row: GenerationModelRow) -> Self {
        Self {
            generation_model_id: row.id,
            kind: AiProviderKind::parse(&row.kind)
                .expect("unknown ai provider kind in generation_models"),
            model: row.model,
        }
    }
}
