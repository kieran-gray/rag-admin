use serde::{Deserialize, Serialize};

use crate::server::domain::shared::event_payloads::RunFingerprintPayload;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunFingerprint {
    pub document_content_hash: String,
    pub dataset_content_hash: String,
    pub embedding_model_snapshot: serde_json::Value,
    pub generation_model_snapshot: serde_json::Value,
}

impl From<RunFingerprint> for RunFingerprintPayload {
    fn from(f: RunFingerprint) -> Self {
        Self {
            document_content_hash: f.document_content_hash,
            dataset_content_hash: f.dataset_content_hash,
            embedding_model_snapshot: f.embedding_model_snapshot,
            generation_model_snapshot: f.generation_model_snapshot,
        }
    }
}

impl From<RunFingerprintPayload> for RunFingerprint {
    fn from(p: RunFingerprintPayload) -> Self {
        Self {
            document_content_hash: p.document_content_hash,
            dataset_content_hash: p.dataset_content_hash,
            embedding_model_snapshot: p.embedding_model_snapshot,
            generation_model_snapshot: p.generation_model_snapshot,
        }
    }
}
