use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    document_type::DocumentType,
    source_ref::SourceRef,
    version::{ContentHash, DocumentMetadata},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentReadModel {
    pub document_id: Uuid,
    pub document_type: DocumentType,
    pub source_ref: SourceRef,
    pub latest_version_number: u32,
    pub latest_content_hash: ContentHash,
    pub latest_metadata: DocumentMetadata,
    pub latest_version_occurred_at: String,
    pub deleted: bool,
}
