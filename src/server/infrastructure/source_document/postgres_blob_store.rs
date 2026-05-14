use async_trait::async_trait;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use crate::server::application::{source_document::ports::BlobStore, AppError};
use crate::server::domain::source_document::version::ContentHash;

pub struct PostgresBlobStore {
    pool: PgPool,
}

impl PostgresBlobStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn compute_hash(content: &[u8]) -> ContentHash {
        let mut hasher = Sha256::new();
        hasher.update(content);
        ContentHash::new(hex::encode(hasher.finalize()))
    }
}

#[async_trait]
impl BlobStore for PostgresBlobStore {
    async fn put(&self, content: &[u8]) -> Result<ContentHash, AppError> {
        let hash = Self::compute_hash(content);
        sqlx::query(
            r#"
            INSERT INTO source_document_blobs (content_hash, bytes, created_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (content_hash) DO NOTHING
            "#,
        )
        .bind(hash.as_hex())
        .bind(content)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("blob store put: {e}")))?;

        Ok(hash)
    }

    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>, AppError> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT bytes FROM source_document_blobs WHERE content_hash = $1")
                .bind(hash.as_hex())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| AppError::Internal(format!("blob store get: {e}")))?;

        row.map(|(bytes,)| bytes)
            .ok_or_else(|| AppError::NotFound(format!("blob not found: {hash}")))
    }

    async fn delete(&self, hash: &ContentHash) -> Result<(), AppError> {
        sqlx::query("DELETE FROM source_document_blobs WHERE content_hash = $1")
            .bind(hash.as_hex())
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(format!("blob store delete: {e}")))?;

        Ok(())
    }
}
