use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunFingerprint {
    pub document_content_hash: String,
    pub dataset_content_hash: String,
    pub embedding_model_snapshot: serde_json::Value,
}
