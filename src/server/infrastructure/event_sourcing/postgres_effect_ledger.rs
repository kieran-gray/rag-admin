use std::marker::PhantomData;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::event_sourcing::effect::{
    EffectLedger, EffectRecord, EffectStatus, IdempotencyKey, PendingEffect,
};

pub struct PostgresEffectLedger<R> {
    pool: PgPool,
    _phantom: PhantomData<R>,
}

impl<R> PostgresEffectLedger<R> {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<R> EffectLedger<R> for PostgresEffectLedger<R>
where
    R: Serialize + DeserializeOwned + Send + Sync,
{
    async fn insert(
        &self,
        aggregate_type: &str,
        effects: &[PendingEffect<R>],
    ) -> Result<(), AppError> {
        if effects.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Internal(format!("begin transaction: {e}")))?;

        for effect in effects {
            let payload = serde_json::to_value(&effect.payload)
                .map_err(|e| AppError::Internal(format!("serialize effect payload: {e}")))?;

            sqlx::query(
                "INSERT INTO pending_effects \
                     (effect_id, aggregate_type, stream_id, event_log_position, \
                      effect_type, effect_payload, idempotency_key) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (idempotency_key) DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(aggregate_type)
            .bind(effect.stream_id)
            .bind(effect.event_log_position)
            .bind(effect.effect_type)
            .bind(&payload)
            .bind(effect.idempotency_key.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(format!("insert effect: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("commit transaction: {e}")))?;
        Ok(())
    }

    async fn pending(
        &self,
        aggregate_type: &str,
        max_attempts: i32,
    ) -> Result<Vec<EffectRecord<R>>, AppError> {
        let rows: Vec<EffectRow> = sqlx::query_as(
            "SELECT effect_id, stream_id, event_log_position, idempotency_key, \
                    status, attempts, effect_payload \
             FROM pending_effects \
             WHERE aggregate_type = $1 \
               AND attempts < $2 \
               AND status IN ('pending', 'failed') \
             ORDER BY event_log_position ASC, effect_id ASC \
             LIMIT 100",
        )
        .bind(aggregate_type)
        .bind(max_attempts)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load pending effects: {e}")))?;

        rows.into_iter()
            .map(|row| {
                let payload: R = serde_json::from_value(row.effect_payload)
                    .map_err(|e| AppError::Internal(format!("deserialize effect payload: {e}")))?;
                Ok(EffectRecord {
                    effect_id: row.effect_id,
                    stream_id: row.stream_id,
                    event_log_position: row.event_log_position,
                    idempotency_key: IdempotencyKey(row.idempotency_key),
                    status: EffectStatus::parse(&row.status),
                    attempts: row.attempts,
                    payload,
                })
            })
            .collect()
    }

    async fn mark_dispatched(&self, effect_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE pending_effects SET status = 'dispatched', last_attempt_at = NOW() \
             WHERE effect_id = $1",
        )
        .bind(effect_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("mark dispatched: {e}")))?;
        Ok(())
    }

    async fn mark_completed(&self, effect_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE pending_effects SET status = 'completed', last_attempt_at = NOW() \
             WHERE effect_id = $1",
        )
        .bind(effect_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("mark completed: {e}")))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        effect_id: Uuid,
        error: &str,
        attempts: i32,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE pending_effects SET status = 'failed', attempts = $2, \
                 last_attempt_at = NOW(), last_error = $3 \
             WHERE effect_id = $1",
        )
        .bind(effect_id)
        .bind(attempts)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("mark failed: {e}")))?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct EffectRow {
    effect_id: Uuid,
    stream_id: Uuid,
    event_log_position: i64,
    idempotency_key: String,
    status: String,
    attempts: i32,
    effect_payload: serde_json::Value,
}
