use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum MapItemRef {
    Observation {
        map_id: Uuid,
        observation_id: Uuid,
    },
    Thread {
        map_id: Uuid,
        thread_id: Uuid,
    },
    Insight {
        map_id: Uuid,
        insight_id: Uuid,
    },
    Connection {
        corpus_map_id: Uuid,
        connection_id: Uuid,
    },
}

impl MapItemRef {
    pub fn tier(&self) -> &'static str {
        match self {
            Self::Observation { .. } => "observation",
            Self::Thread { .. } => "thread",
            Self::Insight { .. } => "insight",
            Self::Connection { .. } => "connection",
        }
    }
}
