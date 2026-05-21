use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::retrieval_profile::{
    RetrievalProfile, RetrievalProfileReadModel, RetrievalProfileRepository,
    RetrievalProfileRepositoryError,
};
use event_sourcing::error::ProjectionError;

pub struct PostgresRetrievalProfileRepository {
    pool: PgPool,
}

impl PostgresRetrievalProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<RetrievalProfile> for PostgresRetrievalProfileRepository {
    async fn upsert(&self, entry: RetrievalProfile) -> Result<(), ProjectionError> {
        sqlx::query(
            "
            INSERT INTO retrieval_profiles (
                id, name, index_profile_id, generation_model_id, reranker_model_id,
                default_top_k, default_min_score_milli
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                name                   = $2,
                index_profile_id       = $3,
                generation_model_id    = $4,
                reranker_model_id      = $5,
                default_top_k          = $6,
                default_min_score_milli = $7,
                updated_at             = NOW()
            ",
        )
        .bind(entry.retrieval_profile_id)
        .bind(&entry.name)
        .bind(entry.index_profile_id)
        .bind(entry.generation_model_id)
        .bind(entry.reranker_model_id)
        .bind(entry.default_top_k as i32)
        .bind(entry.default_min_score_milli as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| ProjectionError::Storage(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM retrieval_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| ProjectionError::Storage(format!("delete: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl RetrievalProfileRepository for PostgresRetrievalProfileRepository {
    async fn load_all(
        &self,
    ) -> Result<Vec<RetrievalProfileReadModel>, RetrievalProfileRepositoryError> {
        let rows: Vec<RetrievalProfileRow> = sqlx::query_as(
            "
            SELECT id, name, index_profile_id, generation_model_id, reranker_model_id,
                   default_top_k, default_min_score_milli
            FROM retrieval_profiles
            ORDER BY created_at ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RetrievalProfileRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<RetrievalProfileReadModel>, RetrievalProfileRepositoryError> {
        let row: Option<RetrievalProfileRow> = sqlx::query_as(
            "
            SELECT id, name, index_profile_id, generation_model_id, reranker_model_id,
                   default_top_k, default_min_score_milli
            FROM retrieval_profiles
            WHERE id = $1
            ",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RetrievalProfileRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }
}

struct RetrievalProfileRow {
    id: Uuid,
    name: String,
    index_profile_id: Uuid,
    generation_model_id: Uuid,
    reranker_model_id: Option<Uuid>,
    default_top_k: i32,
    default_min_score_milli: i32,
}

impl sqlx::FromRow<'_, PgRow> for RetrievalProfileRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            index_profile_id: row.try_get("index_profile_id")?,
            generation_model_id: row.try_get("generation_model_id")?,
            reranker_model_id: row.try_get("reranker_model_id")?,
            default_top_k: row.try_get("default_top_k")?,
            default_min_score_milli: row.try_get("default_min_score_milli")?,
        })
    }
}

impl From<RetrievalProfileRow> for RetrievalProfileReadModel {
    fn from(row: RetrievalProfileRow) -> Self {
        Self {
            retrieval_profile_id: row.id,
            name: row.name,
            index_profile_id: row.index_profile_id,
            generation_model_id: row.generation_model_id,
            reranker_model_id: row.reranker_model_id,
            default_top_k: row.default_top_k as u32,
            default_min_score_milli: row.default_min_score_milli as u32,
        }
    }
}
