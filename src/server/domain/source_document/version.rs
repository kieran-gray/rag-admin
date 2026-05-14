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

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlogPostMetadata {
    pub title: String,
    pub published_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DocumentMetadata {
    BlogPost(BlogPostMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub version_number: u32,
    pub content_hash: ContentHash,
    pub occurred_at: Timestamp,
    pub metadata: DocumentMetadata,
}
