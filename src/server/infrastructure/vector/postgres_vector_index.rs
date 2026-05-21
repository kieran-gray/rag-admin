use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;

use crate::server::application::indexing::ports::vector_index::{
    MetadataFilterOperation, VectorIndex, VectorIndexDescription, VectorMatch, VectorQuery,
};
use crate::server::application::indexing::VectorIndexProviderRegistry;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::application::AppError;
use crate::server::domain::VectorRecord;
use crate::server::infrastructure::shared::sql::pgvector_codec::format_vector_literal;
use crate::shared::reference_data::VectorStoreKind;

pub struct PostgresVectorIndex {
    pool: PgPool,
    index_name: String,
    dimensions: u32,
}

impl PostgresVectorIndex {
    pub fn new(pool: PgPool, index_name: String, dimensions: u32) -> Arc<Self> {
        Arc::new(Self {
            pool,
            index_name,
            dimensions,
        })
    }
}

#[async_trait]
impl VectorIndex for PostgresVectorIndex {
    async fn upsert(&self, records: &[VectorRecord]) -> Result<(), AppError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Internal(format!("pg vector upsert begin: {e}")))?;
        for r in records {
            let literal = format_vector_literal(&r.values);
            sqlx::query(
                "INSERT INTO vector_index_records (index_name, id, vec, metadata)
                 VALUES ($1, $2, $3::vector, $4)
                 ON CONFLICT (index_name, id) DO UPDATE
                   SET vec = EXCLUDED.vec, metadata = EXCLUDED.metadata",
            )
            .bind(&self.index_name)
            .bind(&r.id)
            .bind(literal)
            .bind(&r.metadata)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("pg vector upsert: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("pg vector upsert commit: {e}")))?;
        Ok(())
    }

    async fn delete(&self, ids: &[String]) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        sqlx::query("DELETE FROM vector_index_records WHERE index_name = $1 AND id = ANY($2)")
            .bind(&self.index_name)
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(format!("pg vector delete: {e}")))?;
        Ok(())
    }

    async fn query(&self, q: &VectorQuery) -> Result<Vec<VectorMatch>, AppError> {
        let literal = format_vector_literal(&q.vector);
        let mut sql = String::from(
            "SELECT id, (1 - (vec <=> $1::vector))::real AS score, metadata
             FROM vector_index_records
             WHERE index_name = $2",
        );
        let mut bind_index = 3;
        for f in &q.filter {
            let op = match f.operation {
                MetadataFilterOperation::Equal => "=",
                MetadataFilterOperation::NotEqual => "<>",
            };
            _ = write!(
                sql,
                " AND metadata->>${} {} ${}",
                bind_index,
                op,
                bind_index + 1
            );
            bind_index += 2;
        }
        _ = write!(sql, " ORDER BY vec <=> $1::vector LIMIT ${bind_index}");

        let mut query = sqlx::query_as::<_, (String, f32, Value)>(&sql)
            .bind(literal)
            .bind(&self.index_name);
        for f in &q.filter {
            query = query.bind(&f.field).bind(&f.value);
        }
        query = query.bind(i64::from(q.top_k));

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(format!("pg vector query: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|(id, score, metadata)| VectorMatch {
                id,
                score,
                metadata,
            })
            .collect())
    }

    async fn describe(&self) -> Result<VectorIndexDescription, AppError> {
        Ok(VectorIndexDescription {
            name: self.index_name.clone(),
            dimensions: self.dimensions,
        })
    }
}

pub struct PostgresVectorIndexProvider {
    pool: PgPool,
}

impl PostgresVectorIndexProvider {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

impl VectorIndexProvider for PostgresVectorIndexProvider {
    fn build(&self, index_name: &str, dimensions: u32) -> Arc<dyn VectorIndex> {
        PostgresVectorIndex::new(self.pool.clone(), index_name.to_owned(), dimensions)
    }
}

pub fn register(registry: &mut VectorIndexProviderRegistry, pool: PgPool) {
    registry.register(
        VectorStoreKind::Postgres,
        PostgresVectorIndexProvider::new(pool),
    );
}
