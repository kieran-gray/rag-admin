use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConnectorEvent;
use super::read_model::ConnectorReadModel;
use super::repository::ConnectorRepository;

pub struct ConnectorProjector {
    repository: Arc<dyn ConnectorRepository>,
}

impl ConnectorProjector {
    pub const NAME: &'static str = "connector_projector";

    pub fn new(repository: Arc<dyn ConnectorRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<ConnectorEvent> for ConnectorProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ConnectorEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let connector_id = envelope.metadata.stream_id;
            match &envelope.event {
                ConnectorEvent::ConnectorRegistered(e) => {
                    self.repository
                        .save(ConnectorReadModel {
                            connector_id,
                            name: e.name.clone(),
                            config: e.config.clone(),
                            deleted: false,
                            created_at: e.occurred_at.to_string(),
                            updated_at: e.occurred_at.to_string(),
                        })
                        .await?;
                }
                ConnectorEvent::ConnectorRenamed(e) => {
                    if let Some(mut m) = self.repository.load(connector_id).await? {
                        m.name = e.name.clone();
                        m.updated_at = e.occurred_at.to_string();
                        self.repository.save(m).await?;
                    }
                }
                ConnectorEvent::ConnectorConfigUpdated(e) => {
                    if let Some(mut m) = self.repository.load(connector_id).await? {
                        m.config = e.config.clone();
                        m.updated_at = e.occurred_at.to_string();
                        self.repository.save(m).await?;
                    }
                }
                ConnectorEvent::ConnectorUnregistered(e) => {
                    self.repository
                        .mark_deleted(connector_id, e.occurred_at.to_string())
                        .await?;
                }
            }
        }
        Ok(())
    }
}
