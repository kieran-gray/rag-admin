use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Timings {
    pub embed_ms: u32,
    pub retrieve_ms: u32,
    pub rerank_ms: Option<u32>,
    pub generate_ms: Option<u32>,
    pub total_ms: u32,
}
