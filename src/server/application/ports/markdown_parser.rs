use crate::server::application::markdown::Document;
use crate::server::application::AppError;

pub trait MarkdownParser: Send + Sync {
    fn parse(&self, source: &str) -> Result<Document, AppError>;
}
