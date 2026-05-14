use async_trait::async_trait;

use crate::server::application::AppError;

use super::envelope::EventEnvelope;

#[async_trait]
pub trait Projector<E>: Send + Sync
where
    E: Send + Sync,
{
    fn name(&self) -> &str;

    async fn project(&self, events: &[EventEnvelope<E>]) -> Result<(), AppError>;
}
