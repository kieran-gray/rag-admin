use crate::server::application::source_document::ports::ExtractedDocument;
use crate::server::application::AppError;

pub trait PdfToMarkdown: Send + Sync {
    fn convert(&self, bytes: &[u8]) -> Result<ExtractedDocument, AppError>;
}
