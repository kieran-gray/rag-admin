use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::event_sourcing::error::EsError;
use crate::event_sourcing::job_queue::{ClaimedJob, IdempotencyKey, JobQueue, NewJob};

pub struct PostgresJobQueue<R> {
    pool: PgPool,
    _phantom: PhantomData<R>,
}

impl<R> PostgresJobQueue<R> {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<R> JobQueue<R> for PostgresJobQueue<R>
where
    R: Serialize + DeserializeOwned + Send + Sync,
{
    async fn enqueue(&self, queue: &str, jobs: &[NewJob<R>]) -> Result<(), EsError> {
        if jobs.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| EsError::Storage(format!("begin transaction: {e}")))?;

        for job in jobs {
            let job_id = Uuid::now_v7();
            let payload = serde_json::to_value(&job.payload)
                .map_err(|e| EsError::Storage(format!("serialize job payload: {e}")))?;

            sqlx::query(
                "INSERT INTO jobs (id, queue, partition_key, job_type, payload, idempotency_key) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (queue, idempotency_key) DO NOTHING",
            )
            .bind(job_id)
            .bind(queue)
            .bind(job.partition_key)
            .bind(job.job_type)
            .bind(&payload)
            .bind(job.idempotency_key.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|e| EsError::Storage(format!("insert job: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| EsError::Storage(format!("commit transaction: {e}")))?;
        Ok(())
    }

    async fn claim(
        &self,
        queue: &str,
        worker_id: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedJob<R>>, EsError> {
        let lease_secs = lease.as_secs() as f64;
        let row: Option<ClaimedRow> = sqlx::query_as(
            "WITH candidate AS ( \
                 SELECT id FROM jobs \
                  WHERE queue = $1 \
                    AND run_at <= NOW() \
                    AND (locked_until IS NULL OR locked_until < NOW()) \
                  ORDER BY run_at, created_at \
                  FOR UPDATE SKIP LOCKED \
                  LIMIT 1 \
             ) \
             UPDATE jobs \
                SET locked_until = NOW() + make_interval(secs => $2), \
                    locked_by    = $3, \
                    attempts     = jobs.attempts + 1 \
               FROM candidate \
              WHERE jobs.id = candidate.id \
              RETURNING jobs.id, jobs.partition_key, jobs.job_type, \
                        jobs.idempotency_key, jobs.attempts, jobs.max_attempts, \
                        jobs.payload",
        )
        .bind(queue)
        .bind(lease_secs)
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EsError::Storage(format!("claim job: {e}")))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let payload: R = serde_json::from_value(row.payload)
            .map_err(|e| EsError::Storage(format!("deserialize job payload: {e}")))?;
        Ok(Some(ClaimedJob {
            id: row.id,
            partition_key: row.partition_key,
            job_type: row.job_type,
            idempotency_key: IdempotencyKey(row.idempotency_key),
            attempts: row.attempts,
            max_attempts: row.max_attempts,
            payload,
        }))
    }

    async fn heartbeat(
        &self,
        job_id: Uuid,
        worker_id: &str,
        lease: Duration,
    ) -> Result<bool, EsError> {
        let lease_secs = lease.as_secs() as f64;
        let result = sqlx::query(
            "UPDATE jobs SET locked_until = NOW() + make_interval(secs => $2) \
              WHERE id = $1 AND locked_by = $3",
        )
        .bind(job_id)
        .bind(lease_secs)
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EsError::Storage(format!("heartbeat: {e}")))?;
        Ok(result.rows_affected() == 1)
    }

    async fn ack(&self, job_id: Uuid, worker_id: &str) -> Result<(), EsError> {
        sqlx::query("DELETE FROM jobs WHERE id = $1 AND locked_by = $2")
            .bind(job_id)
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| EsError::Storage(format!("ack: {e}")))?;
        Ok(())
    }

    async fn nack(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error: &str,
        backoff: Duration,
    ) -> Result<(), EsError> {
        let backoff_secs = backoff.as_secs() as f64;
        sqlx::query(
            "UPDATE jobs SET \
                 run_at       = NOW() + make_interval(secs => $2), \
                 locked_until = NULL, \
                 locked_by    = NULL, \
                 last_error   = $3 \
              WHERE id = $1 AND locked_by = $4",
        )
        .bind(job_id)
        .bind(backoff_secs)
        .bind(error)
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EsError::Storage(format!("nack: {e}")))?;
        Ok(())
    }

    async fn dead_letter(&self, job_id: Uuid, error: &str) -> Result<(), EsError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| EsError::Storage(format!("begin transaction: {e}")))?;

        sqlx::query(
            "INSERT INTO dead_jobs (id, queue, partition_key, job_type, payload, \
                                    idempotency_key, attempts, last_error, created_at) \
             SELECT id, queue, partition_key, job_type, payload, \
                    idempotency_key, attempts, $2, created_at \
               FROM jobs WHERE id = $1",
        )
        .bind(job_id)
        .bind(error)
        .execute(&mut *tx)
        .await
        .map_err(|e| EsError::Storage(format!("insert dead_job: {e}")))?;

        sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(job_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| EsError::Storage(format!("delete dead job: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| EsError::Storage(format!("commit transaction: {e}")))?;
        Ok(())
    }

    async fn cancel_partition(&self, queue: &str, partition_key: Uuid) -> Result<u64, EsError> {
        let result = sqlx::query(
            "DELETE FROM jobs \
              WHERE queue = $1 AND partition_key = $2 \
                AND (locked_until IS NULL OR locked_until < NOW())",
        )
        .bind(queue)
        .bind(partition_key)
        .execute(&self.pool)
        .await
        .map_err(|e| EsError::Storage(format!("cancel partition: {e}")))?;
        Ok(result.rows_affected())
    }
}

#[derive(sqlx::FromRow)]
struct ClaimedRow {
    id: Uuid,
    partition_key: Option<Uuid>,
    job_type: String,
    idempotency_key: String,
    attempts: i32,
    max_attempts: i32,
    payload: serde_json::Value,
}
