use std::fmt;

use serde::{Deserialize, Serialize};

use crate::server::domain::shared::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn new(hex: String) -> Self {
        Self(hex)
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlainMetadata {
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebPageMetadata {
    pub title: String,
    pub source_url: String,
    pub fetched_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DocumentMetadata {
    Markdown(PlainMetadata),
    PlainText(PlainMetadata),
    WebPage(WebPageMetadata),
}

impl DocumentMetadata {
    pub fn title(&self) -> &str {
        match self {
            DocumentMetadata::Markdown(m) | DocumentMetadata::PlainText(m) => &m.title,
            DocumentMetadata::WebPage(m) => &m.title,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub version_number: u32,
    pub content_hash: ContentHash,
    pub occurred_at: Timestamp,
    pub metadata: DocumentMetadata,
}
