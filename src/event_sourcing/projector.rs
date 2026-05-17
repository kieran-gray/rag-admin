use async_trait::async_trait;

use super::envelope::EventEnvelope;
use super::error::ProjectionError;

#[async_trait]
pub trait Projector<E>: Send + Sync
where
    E: Send + Sync,
{
    fn name(&self) -> &str;

    async fn project(&self, events: &[EventEnvelope<E>]) -> Result<(), ProjectionError>;
}
