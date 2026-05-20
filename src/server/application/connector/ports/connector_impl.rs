use async_trait::async_trait;

use crate::server::application::source_document::ports::RawDocument;
use crate::server::application::AppError;
use crate::server::domain::connector::{ConnectorConfig, ConnectorKind};
use crate::server::domain::source_document::source_ref::SourceRef;

pub struct DiscoveredItem {
    pub source_ref: SourceRef,
    pub title: String,
}

#[async_trait]
pub trait ConnectorImpl: Send + Sync {
    fn kind(&self) -> ConnectorKind;

    async fn list(&self, config: &ConnectorConfig) -> Result<Vec<DiscoveredItem>, AppError>;

    async fn fetch(
        &self,
        config: &ConnectorConfig,
        source_ref: &SourceRef,
    ) -> Result<RawDocument, AppError>;
}
