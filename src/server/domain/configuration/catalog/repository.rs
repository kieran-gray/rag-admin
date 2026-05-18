use async_trait::async_trait;
use uuid::Uuid;

use event_sourcing::error::ProjectionError;

#[async_trait]
pub trait CatalogRepository<E>: Send + Sync
where
    E: Send + Sync,
{
    async fn upsert(&self, entry: E) -> Result<(), ProjectionError>;
    async fn delete(&self, id: Uuid) -> Result<(), ProjectionError>;
}
