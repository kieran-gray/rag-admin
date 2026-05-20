use crate::server::application::ports::ExtractedDocument;
use crate::server::application::AppError;

pub trait PdfToMarkdown: Send + Sync {
    fn convert(&self, bytes: &[u8]) -> Result<ExtractedDocument, AppError>;
}
