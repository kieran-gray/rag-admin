use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    aggregate::SourceDocument,
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

impl From<&SourceDocument> for SourceDocumentReadModel {
    fn from(doc: &SourceDocument) -> Self {
        let latest = doc
            .latest_version()
            .expect("aggregate must have at least one version");
        Self {
            document_id: doc.document_id,
            document_type: doc.document_type.clone(),
            source_ref: doc.source_ref.clone(),
            latest_version_number: latest.version_number,
            latest_content_hash: latest.content_hash.clone(),
            latest_metadata: latest.metadata.clone(),
            latest_version_occurred_at: latest.occurred_at.to_string(),
            deleted: doc.deleted,
        }
    }
}
