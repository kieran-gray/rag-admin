use crate::server::domain::shared::Timestamp;
use crate::server::domain::source_document::source_ref::SourceRef;

use super::content_type::ContentType;

#[derive(Debug, Clone, Default)]
pub struct AcquisitionHints {
    pub filename: Option<String>,
    pub source_url: Option<String>,
    pub fetched_at: Option<Timestamp>,
    pub original_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RawDocument {
    pub source_ref: SourceRef,
    pub bytes: Vec<u8>,
    pub content_type: ContentType,
    pub hints: AcquisitionHints,
}
