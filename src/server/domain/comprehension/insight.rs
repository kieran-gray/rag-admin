use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::map_item::MapItemRef;
use super::span::Span;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Insight {
    pub insight_id: Uuid,
    pub kind: String,
    pub summary: String,
    pub evidence: Vec<MapItemRef>,
    pub spans: Vec<Span>,
}

impl Insight {
    pub fn normalized_kind(&self) -> String {
        self.kind.trim().to_lowercase()
    }
}
