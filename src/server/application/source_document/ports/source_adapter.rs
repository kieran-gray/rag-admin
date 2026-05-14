use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::domain::source_document::{
    document_type::DocumentType, source_ref::SourceRef, version::DocumentMetadata,
};

pub struct DocumentSummary {
    pub source_ref: SourceRef,
    pub title: String,
}

pub struct FetchedDocument {
    pub source_ref: SourceRef,
    pub content: Vec<u8>,
    pub metadata: DocumentMetadata,
}

#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn document_type(&self) -> DocumentType;
    async fn list(&self) -> Result<Vec<DocumentSummary>, AppError>;
    async fn fetch(&self, source_ref: &SourceRef) -> Result<FetchedDocument, AppError>;
}
