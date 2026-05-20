use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStage {
    Fetching,
    Chunking,
    Embedding,
    Indexing,
}

impl fmt::Display for IngestStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IngestStage::Fetching => write!(f, "fetching"),
            IngestStage::Chunking => write!(f, "chunking"),
            IngestStage::Embedding => write!(f, "embedding"),
            IngestStage::Indexing => write!(f, "indexing"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IndexingStatus {
    Pending,
    Chunking,
    Embedding,
    Indexed,
    Failed { stage: IngestStage },
}

impl IndexingStatus {
    pub fn is_at_least_chunking(&self) -> bool {
        matches!(
            self,
            IndexingStatus::Chunking | IndexingStatus::Embedding | IndexingStatus::Indexed
        )
    }

    pub fn is_at_least_embedding(&self) -> bool {
        matches!(self, IndexingStatus::Embedding | IndexingStatus::Indexed)
    }

    pub fn is_indexed(&self) -> bool {
        matches!(self, IndexingStatus::Indexed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, IndexingStatus::Failed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_stage_display_strings() {
        assert_eq!(IngestStage::Fetching.to_string(), "fetching");
        assert_eq!(IngestStage::Chunking.to_string(), "chunking");
        assert_eq!(IngestStage::Embedding.to_string(), "embedding");
        assert_eq!(IngestStage::Indexing.to_string(), "indexing");
    }

    #[test]
    fn is_at_least_chunking_returns_true_after_chunking_completes() {
        assert!(IndexingStatus::Chunking.is_at_least_chunking());
        assert!(IndexingStatus::Embedding.is_at_least_chunking());
        assert!(IndexingStatus::Indexed.is_at_least_chunking());
    }

    #[test]
    fn is_at_least_chunking_returns_false_for_pending_or_failed() {
        assert!(!IndexingStatus::Pending.is_at_least_chunking());
        assert!(!IndexingStatus::Failed {
            stage: IngestStage::Chunking,
        }
        .is_at_least_chunking());
    }

    #[test]
    fn is_at_least_embedding_excludes_chunking_state() {
        assert!(!IndexingStatus::Pending.is_at_least_embedding());
        assert!(!IndexingStatus::Chunking.is_at_least_embedding());
        assert!(IndexingStatus::Embedding.is_at_least_embedding());
        assert!(IndexingStatus::Indexed.is_at_least_embedding());
    }

    #[test]
    fn is_indexed_only_true_for_indexed() {
        assert!(IndexingStatus::Indexed.is_indexed());
        for s in [
            IndexingStatus::Pending,
            IndexingStatus::Chunking,
            IndexingStatus::Embedding,
            IndexingStatus::Failed {
                stage: IngestStage::Indexing,
            },
        ] {
            assert!(!s.is_indexed());
        }
    }

    #[test]
    fn is_failed_matches_every_failed_stage() {
        for stage in [
            IngestStage::Fetching,
            IngestStage::Chunking,
            IngestStage::Embedding,
            IngestStage::Indexing,
        ] {
            assert!(IndexingStatus::Failed { stage }.is_failed());
        }
        assert!(!IndexingStatus::Pending.is_failed());
        assert!(!IndexingStatus::Indexed.is_failed());
    }
}
