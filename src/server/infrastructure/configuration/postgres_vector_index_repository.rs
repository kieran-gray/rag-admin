use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::catalog::VectorStoreKind;
use crate::event_sourcing::error::ProjectionError;
use crate::server::domain::configuration::catalog::CatalogRepository;
use crate::server::domain::configuration::vector_index::{
    VectorIndex, VectorIndexRepository, VectorIndexRepositoryError,
};

pub struct PostgresVectorIndexRepository {
    pool: PgPool,
}

impl PostgresVectorIndexRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CatalogRepository<VectorIndex> for PostgresVectorIndexRepository {
    async fn upsert(&self, index: VectorIndex) -> Result<(), ProjectionError> {
        sqlx::query(
            "
            INSERT INTO vector_indexes (id, kind, name, dimensions)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                kind       = $2,
                name       = $3,
                dimensions = $4,
                updated_at = NOW()
            ",
        )
        .bind(index.index_id)
        .bind(index.kind.as_str())
        .bind(&index.name)
        .bind(index.dimensions as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| ProjectionError::Storage(format!("save: {e}")))?;
        Ok(())
    }

    async fn delete(&self, index_id: Uuid) -> Result<(), ProjectionError> {
        sqlx::query("DELETE FROM vector_indexes WHERE id = $1")
            .bind(index_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ProjectionError::Storage(format!("delete: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl VectorIndexRepository for PostgresVectorIndexRepository {
    async fn load_all(&self) -> Result<Vec<VectorIndex>, VectorIndexRepositoryError> {
        let rows: Vec<VectorIndexRow> = sqlx::query_as(
            "
            SELECT id, kind, name, dimensions
            FROM vector_indexes
            ORDER BY created_at ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VectorIndexRepositoryError::Internal(format!("load_all: {e}")))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(
        &self,
        index_id: Uuid,
    ) -> Result<Option<VectorIndex>, VectorIndexRepositoryError> {
        let row: Option<VectorIndexRow> =
            sqlx::query_as("SELECT id, kind, name, dimensions FROM vector_indexes WHERE id = $1")
                .bind(index_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| VectorIndexRepositoryError::Internal(format!("find_by_id: {e}")))?;
        Ok(row.map(Into::into))
    }
}

struct VectorIndexRow {
    id: Uuid,
    kind: String,
    name: String,
    dimensions: i32,
}

impl sqlx::FromRow<'_, PgRow> for VectorIndexRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            kind: row.try_get("kind")?,
            name: row.try_get("name")?,
            dimensions: row.try_get("dimensions")?,
        })
    }
}

impl From<VectorIndexRow> for VectorIndex {
    #[expect(
        clippy::expect_used,
        reason = "row.kind comes from a column constrained by the application's enum vocabulary; an unrecognised value indicates DB corruption or schema drift, both of which should fail loudly"
    )]
    fn from(row: VectorIndexRow) -> Self {
        Self {
            index_id: row.id,
            kind: VectorStoreKind::parse(&row.kind)
                .expect("unknown vector store kind in vector_indexes"),
            name: row.name,
            dimensions: row.dimensions as u32,
        }
    }
}
