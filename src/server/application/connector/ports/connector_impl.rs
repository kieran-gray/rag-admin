use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::domain::connector::{ConnectorConfig, ConnectorKind};
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::DocumentMetadata;

pub struct DiscoveredItem {
    pub source_ref: SourceRef,
    pub title: String,
}

pub struct FetchedDocument {
    pub source_ref: SourceRef,
    pub content: Vec<u8>,
    pub metadata: DocumentMetadata,
}

#[async_trait]
pub trait ConnectorImpl: Send + Sync {
    fn kind(&self) -> ConnectorKind;

    async fn list(&self, config: &ConnectorConfig) -> Result<Vec<DiscoveredItem>, AppError>;

    async fn fetch(
        &self,
        config: &ConnectorConfig,
        source_ref: &SourceRef,
    ) -> Result<FetchedDocument, AppError>;
}
