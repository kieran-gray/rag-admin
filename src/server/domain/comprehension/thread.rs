use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::map_item::MapItemRef;
use super::span::Span;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thread {
    pub thread_id: Uuid,
    pub section_sequence: u32,
    pub kind: String,
    pub summary: String,
    pub evidence: Vec<MapItemRef>,
    pub spans: Vec<Span>,
}

impl Thread {
    pub fn normalized_kind(&self) -> String {
        self.kind.trim().to_lowercase()
    }
}
