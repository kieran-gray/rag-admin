use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStage {
    Fetching,
    Chunking,
    Embedding,
    Indexing,
}

impl std::fmt::Display for IngestStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
