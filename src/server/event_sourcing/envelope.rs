use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMetadata {
    pub stream_id: Uuid,
    pub aggregate_type: String,
    pub sequence: i64,
    pub log_position: i64,
    pub event_type: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<E> {
    pub event: E,
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedEvent {
    pub stream_id: Uuid,
    pub aggregate_type: String,
    pub sequence: i64,
    pub log_position: i64,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub occurred_at: Timestamp,
}

impl<E: Serialize> EventEnvelope<E> {
    pub fn to_published(&self) -> Result<PublishedEvent, serde_json::Error> {
        Ok(PublishedEvent {
            stream_id: self.metadata.stream_id,
            aggregate_type: self.metadata.aggregate_type.clone(),
            sequence: self.metadata.sequence,
            log_position: self.metadata.log_position,
            event_type: self.metadata.event_type.clone(),
            event_data: serde_json::to_value(&self.event)?,
            occurred_at: self.metadata.occurred_at.clone(),
        })
    }
}
