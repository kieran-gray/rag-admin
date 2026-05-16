use crate::server::application::AppError;

pub struct ExtractedDocument {
    pub title: Option<String>,
    pub markdown: String,
}

pub trait HtmlToMarkdown: Send + Sync {
    fn convert(&self, html: &str) -> Result<ExtractedDocument, AppError>;
}
