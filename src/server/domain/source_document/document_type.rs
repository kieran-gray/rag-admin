use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    BlogPost,
    Markdown,
    PlainText,
    WebPage,
}

impl DocumentType {
    pub fn display_label(&self) -> &'static str {
        match self {
            DocumentType::BlogPost => "Blog post",
            DocumentType::Markdown => "Markdown",
            DocumentType::PlainText => "Plain text",
            DocumentType::WebPage => "Web page",
        }
    }
}
