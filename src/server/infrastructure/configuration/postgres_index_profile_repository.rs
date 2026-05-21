use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::index_profile::{
    IndexProfile, IndexProfileReadModel, IndexProfileRepository, IndexProfileRepositoryError,
};
use event_sourcing::error::ProjectionError;

pub struct PostgresIndexProfileRepository {
    pool: PgPool,
}

impl PostgresIndexProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<IndexProfile> for PostgresIndexProfileRepository {
    async fn upsert(&self, entry: IndexProfile) -> Result<(), ProjectionError> {
        sqlx::query(
            "
            INSERT INTO index_profiles (id, name, embedding_model_id, vector_index_id, dimensions)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                name              = $2,
                embedding_model_id = $3,
                vector_index_id   = $4,
                dimensions        = $5,
                updated_at        = NOW()
            ",
        )
        .bind(entry.index_profile_id)
        .bind(&entry.name)
        .bind(entry.embedding_model_id)
        .bind(entry.vector_index_id)
        .bind(entry.dimensions as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| ProjectionError::Storage(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM index_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| ProjectionError::Storage(format!("delete: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl IndexProfileRepository for PostgresIndexProfileRepository {
    async fn load_all(&self) -> Result<Vec<IndexProfileReadModel>, IndexProfileRepositoryError> {
        let rows: Vec<IndexProfileRow> = sqlx::query_as(
            "
            SELECT id, name, embedding_model_id, vector_index_id, dimensions
            FROM index_profiles
            ORDER BY created_at ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| IndexProfileRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<IndexProfileReadModel>, IndexProfileRepositoryError> {
        let row: Option<IndexProfileRow> = sqlx::query_as(
            "
            SELECT id, name, embedding_model_id, vector_index_id, dimensions
            FROM index_profiles
            WHERE id = $1
            ",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| IndexProfileRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }
}

struct IndexProfileRow {
    id: Uuid,
    name: String,
    embedding_model_id: Uuid,
    vector_index_id: Uuid,
    dimensions: i32,
}

impl sqlx::FromRow<'_, PgRow> for IndexProfileRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            embedding_model_id: row.try_get("embedding_model_id")?,
            vector_index_id: row.try_get("vector_index_id")?,
            dimensions: row.try_get("dimensions")?,
        })
    }
}

impl From<IndexProfileRow> for IndexProfileReadModel {
    fn from(row: IndexProfileRow) -> Self {
        Self {
            index_profile_id: row.id,
            name: row.name,
            embedding_model_id: row.embedding_model_id,
            vector_index_id: row.vector_index_id,
            dimensions: row.dimensions as u32,
        }
    }
}
