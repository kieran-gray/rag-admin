use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;

use crate::server::application::indexing::ports::KvStore;
use crate::server::application::AppError;

pub struct PostgresKvStore {
    pool: PgPool,
}

impl PostgresKvStore {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl KvStore for PostgresKvStore {
    async fn put_json(&self, key: &str, value: &Value) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO kv_store (key, value, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (key) DO UPDATE
               SET value = EXCLUDED.value, updated_at = NOW()",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("pg kv put: {e}")))?;
        Ok(())
    }
}
