use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::source_document::ports::{
    ContentType, DocumentNormalizer, NormalizedDocument, RawDocument,
};
use crate::server::application::AppError;
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::version::{DocumentMetadata, PlainMetadata};

use super::title::title_from_hints;

pub struct PlainTextNormalizer;

impl PlainTextNormalizer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl DocumentNormalizer for PlainTextNormalizer {
    fn content_type(&self) -> ContentType {
        ContentType::PlainText
    }

    async fn normalize(&self, raw: RawDocument) -> Result<NormalizedDocument, AppError> {
        let title = title_from_hints(&raw);
        Ok(NormalizedDocument {
            source_ref: raw.source_ref,
            document_type: DocumentType::PlainText,
            content: raw.bytes,
            metadata: DocumentMetadata::PlainText(PlainMetadata { title }),
        })
    }
}
