use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::source_document::ports::{
    ContentType, DocumentNormalizer, NormalizedDocument, PdfToMarkdown, RawDocument,
};
use crate::server::application::AppError;
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::version::{DocumentMetadata, PdfMetadata};

pub struct PdfNormalizer {
    pdf_to_markdown: Arc<dyn PdfToMarkdown>,
}

impl PdfNormalizer {
    pub fn new(pdf_to_markdown: Arc<dyn PdfToMarkdown>) -> Arc<Self> {
        Arc::new(Self { pdf_to_markdown })
    }
}

#[async_trait]
impl DocumentNormalizer for PdfNormalizer {
    fn content_type(&self) -> ContentType {
        ContentType::Pdf
    }

    async fn normalize(&self, raw: RawDocument) -> Result<NormalizedDocument, AppError> {
        let extracted = self.pdf_to_markdown.convert(&raw.bytes)?;

        let title = extracted
            .title
            .or_else(|| raw.hints.original_title.clone())
            .or_else(|| filename_stem(raw.hints.filename.as_deref()))
            .unwrap_or_else(|| "Untitled PDF".to_string());

        Ok(NormalizedDocument {
            source_ref: raw.source_ref,
            document_type: DocumentType::Pdf,
            content: extracted.markdown.into_bytes(),
            metadata: DocumentMetadata::Pdf(PdfMetadata {
                title,
                original_filename: raw.hints.filename.clone(),
            }),
        })
    }
}

fn filename_stem(filename: Option<&str>) -> Option<String> {
    let filename = filename?;
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
