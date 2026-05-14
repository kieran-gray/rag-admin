use async_trait::async_trait;
use uuid::Uuid;

use crate::server::application::AppError;

use super::envelope::EventEnvelope;

pub type AppendedEvent<E> = EventEnvelope<E>;

#[async_trait]
pub trait EventStore<E>: Send + Sync
where
    E: Send + Sync,
{
    async fn load(&self, stream_id: Uuid) -> Result<Vec<EventEnvelope<E>>, AppError>;

    async fn load_after(
        &self,
        stream_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<EventEnvelope<E>>, AppError>;

    async fn append(
        &self,
        stream_id: Uuid,
        expected_version: usize,
        events: &[E],
    ) -> Result<Vec<AppendedEvent<E>>, AppError>;

    async fn load_global_after(
        &self,
        after_log_position: i64,
        limit: i64,
    ) -> Result<Vec<EventEnvelope<E>>, AppError>;
}
