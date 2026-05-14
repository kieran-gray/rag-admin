use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::application::{
    evaluation::ports::retriever::{RetrievalQuery, RetrievedChunk, Retriever},
    AppError,
};
use crate::server::infrastructure::postgres::pgvector_codec::format_vector_literal;

pub struct PgvectorRetriever {
    pool: PgPool,
}

impl PgvectorRetriever {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Retriever for PgvectorRetriever {
    async fn retrieve(&self, q: &RetrievalQuery) -> Result<Vec<RetrievedChunk>, AppError> {
        let literal = format_vector_literal(&q.query_vector);

        let rows: Vec<(Uuid, f32)> = sqlx::query_as(
            "SELECT chunk_id, (1 - (vec <=> $1::vector))::real AS score
             FROM chunk_embeddings
             WHERE embedding_set_id = $2
               AND (1 - (vec <=> $1::vector)) >= $4
             ORDER BY vec <=> $1::vector
             LIMIT $3",
        )
        .bind(literal)
        .bind(q.embedding_set_id)
        .bind(q.top_k as i64)
        .bind(q.min_score)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("pgvector retrieve: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|(chunk_id, score)| RetrievedChunk { chunk_id, score })
            .collect())
    }
}
