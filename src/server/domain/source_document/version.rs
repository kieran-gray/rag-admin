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
    pub slug: String,
    pub fetched_at: Timestamp,
}

pub fn slug_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let after_scheme = trimmed.find("//").map(|i| i + 2).unwrap_or(0);
    let host_and_path = trimmed.get(after_scheme..).unwrap_or(trimmed);
    let path = host_and_path
        .find('/')
        .map_or("", |i| host_and_path.get(i + 1..).unwrap_or(""));
    let last_segment = path.rsplit('/').find(|seg| !seg.is_empty());
    match last_segment {
        Some(seg) => seg.to_string(),
        None => trimmed.to_string(),
    }
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

    pub fn slug(&self) -> Option<&str> {
        match self {
            DocumentMetadata::Markdown(_) | DocumentMetadata::PlainText(_) => None,
            DocumentMetadata::WebPage(m) => Some(&m.slug),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_url_takes_last_path_segment() {
        assert_eq!(
            slug_from_url("https://kgdev.me/posts/quest-exactly-once-p1/"),
            "quest-exactly-once-p1"
        );
        assert_eq!(
            slug_from_url("https://kgdev.me/glossary/event-sourcing"),
            "event-sourcing"
        );
        assert_eq!(slug_from_url("https://kgdev.me/posts/a/b/c/"), "c");
    }

    #[test]
    fn slug_from_url_falls_back_when_no_path() {
        assert_eq!(slug_from_url("https://kgdev.me"), "https://kgdev.me");
        assert_eq!(slug_from_url("https://kgdev.me/"), "https://kgdev.me");
    }
}
