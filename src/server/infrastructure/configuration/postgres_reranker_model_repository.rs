use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::reranker_model::{
    RerankerModel, RerankerModelRepository, RerankerModelRepositoryError,
};
use crate::shared::reference_data::AiProviderKind;
use event_sourcing::error::ProjectionError;

pub struct PostgresRerankerModelRepository {
    pool: PgPool,
}

impl PostgresRerankerModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<RerankerModel> for PostgresRerankerModelRepository {
    async fn upsert(&self, model: RerankerModel) -> Result<(), ProjectionError> {
        sqlx::query(
            "
            INSERT INTO reranker_models (id, kind, model)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET
                kind       = $2,
                model      = $3,
                updated_at = NOW()
            ",
        )
        .bind(model.reranker_model_id)
        .bind(model.kind.as_str())
        .bind(&model.model)
        .execute(&self.pool)
        .await
        .map_err(|e| ProjectionError::Storage(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, model_id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM reranker_models WHERE id = $1")
            .bind(model_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ProjectionError::Storage(format!("delete: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl RerankerModelRepository for PostgresRerankerModelRepository {
    async fn load_all(&self) -> Result<Vec<RerankerModel>, RerankerModelRepositoryError> {
        let rows: Vec<RerankerModelRow> = sqlx::query_as(
            "
            SELECT id, kind, model
            FROM reranker_models
            ORDER BY created_at ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RerankerModelRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        model_id: Uuid,
    ) -> Result<Option<RerankerModel>, RerankerModelRepositoryError> {
        let row: Option<RerankerModelRow> =
            sqlx::query_as("SELECT id, kind, model FROM reranker_models WHERE id = $1")
                .bind(model_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    RerankerModelRepositoryError::Internal(format!("find_by_id: {e}"))
                })?;
        Ok(row.map(Into::into))
    }
}

struct RerankerModelRow {
    id: Uuid,
    kind: String,
    model: String,
}

impl sqlx::FromRow<'_, PgRow> for RerankerModelRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            kind: row.try_get("kind")?,
            model: row.try_get("model")?,
        })
    }
}

impl From<RerankerModelRow> for RerankerModel {
    #[expect(
        clippy::expect_used,
        reason = "row.kind comes from a column constrained by the application's enum vocabulary; an unrecognised value indicates DB corruption or schema drift, both of which should fail loudly"
    )]
    fn from(row: RerankerModelRow) -> Self {
        Self {
            reranker_model_id: row.id,
            kind: AiProviderKind::parse(&row.kind)
                .expect("unknown ai provider kind in reranker_models"),
            model: row.model,
        }
    }
}
