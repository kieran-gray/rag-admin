use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl PublishedEvent {
    pub fn from_any(&self, aggregate_types: &[&str]) -> bool {
        aggregate_types.contains(&self.aggregate_type.as_str())
    }
}
