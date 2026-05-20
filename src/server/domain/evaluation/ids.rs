use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatasetId(pub Uuid);

impl DatasetId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for DatasetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for DatasetId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub Uuid);

impl RunId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for RunId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_id_construction_and_extraction_roundtrip() {
        let raw = Uuid::new_v4();
        let id = DatasetId::new(raw);
        assert_eq!(id.as_uuid(), raw);
        assert_eq!(DatasetId::from(raw), id);
    }

    #[test]
    fn run_id_construction_and_extraction_roundtrip() {
        let raw = Uuid::new_v4();
        let id = RunId::new(raw);
        assert_eq!(id.as_uuid(), raw);
        assert_eq!(RunId::from(raw), id);
    }

    #[test]
    fn display_matches_inner_uuid() {
        let raw = Uuid::nil();
        assert_eq!(DatasetId::new(raw).to_string(), raw.to_string());
        assert_eq!(RunId::new(raw).to_string(), raw.to_string());
    }

    #[test]
    fn dataset_id_and_run_id_with_same_uuid_are_distinct_types_but_round_trip_via_serde() {
        let raw = Uuid::new_v4();
        let dataset = DatasetId::new(raw);
        let run = RunId::new(raw);
        let dj = serde_json::to_string(&dataset).expect("dataset json");
        let rj = serde_json::to_string(&run).expect("run json");
        assert_eq!(dj, rj);
        let back: DatasetId = serde_json::from_str(&dj).expect("dataset back");
        assert_eq!(back, dataset);
    }
}
