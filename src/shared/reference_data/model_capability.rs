use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Embedding,
    Generation,
    Reranker,
}

impl ModelCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelCapability::Embedding => "embedding",
            ModelCapability::Generation => "generation",
            ModelCapability::Reranker => "reranker",
        }
    }
}
