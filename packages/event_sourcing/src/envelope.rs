use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMetadata {
    pub stream_id: Uuid,
    pub aggregate_type: String,
    pub sequence: i64,
    pub log_position: i64,
    pub event_type: String,
    pub occurred_at: String,
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
    pub occurred_at: String,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type")]
    enum Ev {
        Happened,
    }

    fn env(sequence: i64, log_position: i64) -> EventEnvelope<Ev> {
        EventEnvelope {
            event: Ev::Happened,
            metadata: EventMetadata {
                stream_id: Uuid::nil(),
                aggregate_type: "agg".into(),
                sequence,
                log_position,
                event_type: "Happened".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn to_published_copies_all_metadata_fields() {
        let p = env(3, 99).to_published().expect("publish");
        assert_eq!(p.stream_id, Uuid::nil());
        assert_eq!(p.aggregate_type, "agg");
        assert_eq!(p.sequence, 3);
        assert_eq!(p.log_position, 99);
        assert_eq!(p.event_type, "Happened");
        assert_eq!(p.occurred_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn to_published_serializes_event_payload() {
        let p = env(1, 1).to_published().expect("publish");
        assert_eq!(p.event_data["type"], "Happened");
    }
}
