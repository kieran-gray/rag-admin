use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Span {
    pub document_id: Uuid,
    pub char_start: u32,
    pub char_end: u32,
}

impl Span {
    pub fn new(document_id: Uuid, char_start: u32, char_end: u32) -> Self {
        Self {
            document_id,
            char_start,
            char_end,
        }
    }

    pub fn len(&self) -> u32 {
        self.char_end.saturating_sub(self.char_start)
    }

    pub fn is_empty(&self) -> bool {
        self.char_end <= self.char_start
    }

    pub fn contains(&self, other: &Span) -> bool {
        self.document_id == other.document_id
            && other.char_start >= self.char_start
            && other.char_end <= self.char_end
    }
}
