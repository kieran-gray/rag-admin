use std::marker::PhantomData;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::shared::Timestamp;
use crate::server::event_sourcing::envelope::{EventEnvelope, EventMetadata};
use crate::server::event_sourcing::event_store::{AppendedEvent, EventStore};

pub struct PostgresEventStore<E> {
    pool: PgPool,
    aggregate_type: &'static str,
    _phantom: PhantomData<E>,
}

impl<E> PostgresEventStore<E> {
    pub fn new(pool: PgPool, aggregate_type: &'static str) -> Self {
        Self {
            pool,
            aggregate_type,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<E> EventStore<E> for PostgresEventStore<E>
where
    E: Serialize + DeserializeOwned + Clone + Send + Sync,
{
    async fn load(&self, stream_id: Uuid) -> Result<Vec<EventEnvelope<E>>, AppError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT id, stream_id, aggregate_type, position, event_type, event_data, occurred_at \
             FROM events \
             WHERE stream_id = $1 AND aggregate_type = $2 \
             ORDER BY position ASC",
        )
        .bind(stream_id)
        .bind(self.aggregate_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load events: {e}")))?;

        rows.into_iter().map(EventRow::into_envelope).collect()
    }

    async fn load_after(
        &self,
        stream_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<EventEnvelope<E>>, AppError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT id, stream_id, aggregate_type, position, event_type, event_data, occurred_at \
             FROM events \
             WHERE stream_id = $1 AND aggregate_type = $2 AND position > $3 \
             ORDER BY position ASC",
        )
        .bind(stream_id)
        .bind(self.aggregate_type)
        .bind(after_sequence)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load events after: {e}")))?;

        rows.into_iter().map(EventRow::into_envelope).collect()
    }

    async fn append(
        &self,
        stream_id: Uuid,
        expected_version: usize,
        events: &[E],
    ) -> Result<Vec<AppendedEvent<E>>, AppError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Internal(format!("begin transaction: {e}")))?;

        let (current_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM events WHERE stream_id = $1 AND aggregate_type = $2",
        )
        .bind(stream_id)
        .bind(self.aggregate_type)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("read stream version: {e}")))?;

        let current_count =
            usize::try_from(current_count).map_err(|e| AppError::Internal(e.to_string()))?;

        if current_count != expected_version {
            return Err(AppError::Validation(format!(
                "{} stream {stream_id} version conflict: expected {expected_version}, actual {current_count}",
                self.aggregate_type
            )));
        }

        let mut appended: Vec<AppendedEvent<E>> = Vec::with_capacity(events.len());
        for (i, event) in events.iter().enumerate() {
            let position = (expected_version + i + 1) as i64;
            let json = serde_json::to_value(event)
                .map_err(|e| AppError::Internal(format!("serialize event: {e}")))?;
            let event_type = event_type_from_json(&json);

            let row: (i64, time::OffsetDateTime) = sqlx::query_as(
                "INSERT INTO events (stream_id, aggregate_type, position, event_type, event_data) \
                 VALUES ($1, $2, $3, $4, $5) \
                 RETURNING id, occurred_at",
            )
            .bind(stream_id)
            .bind(self.aggregate_type)
            .bind(position)
            .bind(&event_type)
            .bind(&json)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.constraint() == Some("events_stream_position_unique") {
                        return AppError::Validation(format!(
                            "{} stream version conflict at position {position}",
                            self.aggregate_type
                        ));
                    }
                }
                AppError::Internal(format!("append event: {e}"))
            })?;

            appended.push(EventEnvelope {
                event: event.clone(),
                metadata: EventMetadata {
                    stream_id,
                    aggregate_type: self.aggregate_type.to_string(),
                    sequence: position,
                    log_position: row.0,
                    event_type,
                    occurred_at: format_timestamp(row.1)?,
                },
            });
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Internal(format!("commit transaction: {e}")))?;

        Ok(appended)
    }

    async fn load_global_after(
        &self,
        after_log_position: i64,
        limit: i64,
    ) -> Result<Vec<EventEnvelope<E>>, AppError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT id, stream_id, aggregate_type, position, event_type, event_data, occurred_at \
             FROM events \
             WHERE aggregate_type = $1 AND id > $2 \
             ORDER BY id ASC \
             LIMIT $3",
        )
        .bind(self.aggregate_type)
        .bind(after_log_position)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("load events global: {e}")))?;

        rows.into_iter().map(EventRow::into_envelope).collect()
    }
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: i64,
    stream_id: Uuid,
    aggregate_type: String,
    position: i64,
    event_type: String,
    event_data: serde_json::Value,
    occurred_at: time::OffsetDateTime,
}

impl EventRow {
    fn into_envelope<E: DeserializeOwned>(self) -> Result<EventEnvelope<E>, AppError> {
        let event = serde_json::from_value::<E>(self.event_data)
            .map_err(|e| AppError::Internal(format!("deserialize event: {e}")))?;
        Ok(EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id: self.stream_id,
                aggregate_type: self.aggregate_type,
                sequence: self.position,
                log_position: self.id,
                event_type: self.event_type,
                occurred_at: format_timestamp(self.occurred_at)?,
            },
        })
    }
}

fn event_type_from_json(value: &serde_json::Value) -> String {
    value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn format_timestamp(odt: time::OffsetDateTime) -> Result<Timestamp, AppError> {
    odt.format(&Rfc3339)
        .map(Timestamp::from)
        .map_err(|e| AppError::Internal(format!("format timestamp: {e}")))
}
