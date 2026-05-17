use std::sync::Arc;

use async_trait::async_trait;

use crate::event_sourcing::envelope::EventEnvelope;
use crate::event_sourcing::error::ProjectionError;
use crate::event_sourcing::projector::Projector;

use super::events::SourceDocumentEvent;
use super::read_model::SourceDocumentReadModel;
use super::repository::SourceDocumentRepository;

pub struct SourceDocumentProjector {
    repository: Arc<dyn SourceDocumentRepository>,
}

impl SourceDocumentProjector {
    pub const NAME: &'static str = "source_document_projector";

    pub fn new(repository: Arc<dyn SourceDocumentRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<SourceDocumentEvent> for SourceDocumentProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<SourceDocumentEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let document_id = envelope.metadata.stream_id;
            match &envelope.event {
                SourceDocumentEvent::DocumentCreated(_) => {}
                SourceDocumentEvent::VersionAdded(e) => {
                    let existing = self.repository.load(document_id).await?;
                    let read_model = if let Some(mut m) = existing {
                        m.latest_version_number = e.version_number;
                        m.latest_content_hash = e.content_hash.clone();
                        m.latest_metadata = e.metadata.clone();
                        m.latest_version_occurred_at = e.occurred_at.to_string();
                        m
                    } else {
                        let created = events
                            .iter()
                            .filter(|env| {
                                env.metadata.stream_id == document_id
                                    && env.metadata.log_position < envelope.metadata.log_position
                            })
                            .filter_map(|env| match &env.event {
                                SourceDocumentEvent::DocumentCreated(c) => Some(c),
                                _ => None,
                            })
                            .next_back()
                            .ok_or_else(|| {
                                ProjectionError::Storage(format!(
                                    "VersionAdded for {document_id} without prior DocumentCreated"
                                ))
                            })?;
                        SourceDocumentReadModel {
                            document_id,
                            document_type: created.document_type.clone(),
                            source_ref: created.source_ref.clone(),
                            latest_version_number: e.version_number,
                            latest_content_hash: e.content_hash.clone(),
                            latest_metadata: e.metadata.clone(),
                            latest_version_occurred_at: e.occurred_at.to_string(),
                            deleted: false,
                        }
                    };
                    self.repository.save(read_model).await?;
                }
                SourceDocumentEvent::DocumentDeleted(_) => {
                    if let Some(mut m) = self.repository.load(document_id).await? {
                        m.deleted = true;
                        self.repository.save(m).await?;
                    }
                }
            }
        }
        Ok(())
    }
}
