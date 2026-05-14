use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::{
    document_type::DocumentType,
    source_ref::SourceRef,
    version::{ContentHash, DocumentMetadata},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentCreated {
    pub document_id: Uuid,
    pub document_type: DocumentType,
    pub source_ref: SourceRef,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionAdded {
    pub version_number: u32,
    pub content_hash: ContentHash,
    pub metadata: DocumentMetadata,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentDeleted {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum SourceDocumentEvent {
    DocumentCreated(DocumentCreated),
    VersionAdded(VersionAdded),
    DocumentDeleted(DocumentDeleted),
}
