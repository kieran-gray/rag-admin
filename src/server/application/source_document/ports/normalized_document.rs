use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::DocumentMetadata;

#[derive(Debug, Clone)]
pub struct NormalizedDocument {
    pub source_ref: SourceRef,
    pub document_type: DocumentType,
    pub content: Vec<u8>,
    pub metadata: DocumentMetadata,
}
