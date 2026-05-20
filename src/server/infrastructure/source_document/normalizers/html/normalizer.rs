use std::str;
use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::ports::{Clock, HtmlToMarkdown};
use crate::server::application::source_document::ports::{
    ContentType, DocumentNormalizer, NormalizedDocument, RawDocument,
};
use crate::server::application::AppError;
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::version::{
    humanize_url, slug_from_url, DocumentMetadata, PlainMetadata, WebPageMetadata,
};

use super::title::title_from_hints;

pub struct HtmlNormalizer {
    html_to_markdown: Arc<dyn HtmlToMarkdown>,
    clock: Arc<dyn Clock>,
}

impl HtmlNormalizer {
    pub fn new(html_to_markdown: Arc<dyn HtmlToMarkdown>, clock: Arc<dyn Clock>) -> Arc<Self> {
        Arc::new(Self {
            html_to_markdown,
            clock,
        })
    }
}

#[async_trait]
impl DocumentNormalizer for HtmlNormalizer {
    fn content_type(&self) -> ContentType {
        ContentType::Html
    }

    async fn normalize(&self, raw: RawDocument) -> Result<NormalizedDocument, AppError> {
        let html = str::from_utf8(&raw.bytes)
            .map_err(|e| AppError::Validation(format!("HTML body is not valid utf-8: {e}")))?;
        let extracted = self.html_to_markdown.convert(html)?;
        let extracted_title = extracted.title.clone();
        let markdown_bytes = extracted.markdown.into_bytes();

        if let Some(source_url) = raw.hints.source_url.clone() {
            let title = extracted_title
                .or_else(|| raw.hints.original_title.clone())
                .unwrap_or_else(|| humanize_url(&source_url));
            let fetched_at = raw
                .hints
                .fetched_at
                .clone()
                .unwrap_or_else(|| self.clock.now());
            Ok(NormalizedDocument {
                source_ref: raw.source_ref,
                document_type: DocumentType::WebPage,
                content: markdown_bytes,
                metadata: DocumentMetadata::WebPage(WebPageMetadata {
                    title,
                    slug: slug_from_url(&source_url),
                    source_url,
                    fetched_at,
                }),
            })
        } else {
            let title = extracted_title.unwrap_or_else(|| title_from_hints(&raw));
            Ok(NormalizedDocument {
                source_ref: raw.source_ref,
                document_type: DocumentType::Markdown,
                content: markdown_bytes,
                metadata: DocumentMetadata::Markdown(PlainMetadata { title }),
            })
        }
    }
}
