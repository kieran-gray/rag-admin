use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::repository::CatalogRepository;

pub enum CatalogProjectorAction<E> {
    Noop,
    Upsert(E),
    Remove(Uuid),
}

pub struct CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync,
{
    name: &'static str,
    repository: Arc<dyn CatalogRepository<Entry>>,
    classify: fn(&Event) -> CatalogProjectorAction<Entry>,
}

impl<Event, Entry> CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync,
{
    pub fn new(
        name: &'static str,
        repository: Arc<dyn CatalogRepository<Entry>>,
        classify: fn(&Event) -> CatalogProjectorAction<Entry>,
    ) -> Self {
        Self {
            name,
            repository,
            classify,
        }
    }
}

#[async_trait]
impl<Event, Entry> Projector<Event> for CatalogProjector<Event, Entry>
where
    Event: Send + Sync,
    Entry: Send + Sync + 'static,
{
    fn name(&self) -> &str {
        self.name
    }

    async fn project(&self, events: &[EventEnvelope<Event>]) -> Result<(), ProjectionError> {
        for envelope in events {
            match (self.classify)(&envelope.event) {
                CatalogProjectorAction::Noop => {}
                CatalogProjectorAction::Upsert(entry) => self.repository.upsert(entry).await?,
                CatalogProjectorAction::Remove(id) => self.repository.delete(id).await?,
            }
        }
        Ok(())
    }
}
