use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use super::{read_model::SourceDocumentReadModel, source_ref::SourceRef};
use event_sourcing::error::ProjectionError;

#[derive(Debug, Error)]
pub enum SourceDocumentRepositoryError {
    #[error("source document repository error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFilter {
    Upload,
    UrlHost(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentStatusFilter {
    Registered,
    InProgress,
    Indexed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct DocumentListCursor {
    pub updated_at: String,
    pub document_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentListQuery {
    pub cursor: Option<DocumentListCursor>,
    pub limit: u32,
    pub search: Option<String>,
    pub sources: Vec<SourceFilter>,
    pub statuses: Vec<DocumentStatusFilter>,
}

#[derive(Debug, Clone)]
pub struct DocumentListEntry {
    pub document: SourceDocumentReadModel,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct DocumentListPage {
    pub items: Vec<DocumentListEntry>,
    pub next_cursor: Option<DocumentListCursor>,
    pub total_matching: u64,
    pub total_all: u64,
}

#[async_trait]
pub trait SourceDocumentRepository: Send + Sync {
    async fn load(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError>;

    async fn save(
        &self,
        read_model: SourceDocumentReadModel,
    ) -> Result<(), SourceDocumentRepositoryError>;

    async fn list(&self) -> Result<Vec<SourceDocumentReadModel>, SourceDocumentRepositoryError>;

    async fn list_page(
        &self,
        query: &DocumentListQuery,
    ) -> Result<DocumentListPage, SourceDocumentRepositoryError>;

    async fn source_facets(
        &self,
    ) -> Result<Vec<(SourceFilter, u64)>, SourceDocumentRepositoryError>;

    async fn find_by_source_ref(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError>;
}

impl From<SourceDocumentRepositoryError> for ProjectionError {
    fn from(value: SourceDocumentRepositoryError) -> Self {
        Self::Storage(value.to_string())
    }
}
