use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::span::Span;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Observation {
    pub observation_id: Uuid,
    pub chunk_sequence: u32,
    pub kind: String,
    pub summary: String,
    pub spans: Vec<Span>,
}

impl Observation {
    pub fn normalized_kind(&self) -> String {
        self.kind.trim().to_lowercase()
    }
}
