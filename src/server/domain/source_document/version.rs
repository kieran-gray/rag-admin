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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfMetadata {
    pub title: String,
    #[serde(default)]
    pub original_filename: Option<String>,
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

pub fn humanize_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let after_scheme = trimmed.find("//").map(|i| i + 2).unwrap_or(0);
    let host_and_path = trimmed.get(after_scheme..).unwrap_or(trimmed);
    if let Some(slash) = host_and_path.find('/') {
        let path = host_and_path.get(slash..).unwrap_or("");
        let path = path.trim_start_matches('/');
        if !path.is_empty() {
            return path.to_string();
        }
    }
    url.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DocumentMetadata {
    Markdown(PlainMetadata),
    PlainText(PlainMetadata),
    WebPage(WebPageMetadata),
    Pdf(PdfMetadata),
}

impl DocumentMetadata {
    pub fn title(&self) -> &str {
        match self {
            DocumentMetadata::Markdown(m) | DocumentMetadata::PlainText(m) => &m.title,
            DocumentMetadata::WebPage(m) => &m.title,
            DocumentMetadata::Pdf(m) => &m.title,
        }
    }

    pub fn slug(&self) -> Option<&str> {
        match self {
            DocumentMetadata::Markdown(_)
            | DocumentMetadata::PlainText(_)
            | DocumentMetadata::Pdf(_) => None,
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

    #[test]
    fn humanize_url_extracts_path() {
        assert_eq!(
            humanize_url("https://example.com/blog/my-post/"),
            "blog/my-post"
        );
        assert_eq!(humanize_url("https://example.com"), "https://example.com");
    }

    #[test]
    fn document_metadata_title_returns_inner_title_for_each_variant() {
        let md = DocumentMetadata::Markdown(PlainMetadata { title: "md".into() });
        assert_eq!(md.title(), "md");

        let txt = DocumentMetadata::PlainText(PlainMetadata {
            title: "txt".into(),
        });
        assert_eq!(txt.title(), "txt");

        let web = DocumentMetadata::WebPage(WebPageMetadata {
            title: "web".into(),
            source_url: "https://x".into(),
            slug: "x".into(),
            fetched_at: "2024".into(),
        });
        assert_eq!(web.title(), "web");

        let pdf = DocumentMetadata::Pdf(PdfMetadata {
            title: "doc.pdf".into(),
            original_filename: Some("doc.pdf".into()),
        });
        assert_eq!(pdf.title(), "doc.pdf");
    }

    #[test]
    fn document_metadata_slug_only_present_for_web_page() {
        let web = DocumentMetadata::WebPage(WebPageMetadata {
            title: "t".into(),
            source_url: "https://x/a".into(),
            slug: "a".into(),
            fetched_at: "2024".into(),
        });
        assert_eq!(web.slug(), Some("a"));

        let md = DocumentMetadata::Markdown(PlainMetadata { title: "t".into() });
        assert_eq!(md.slug(), None);

        let txt = DocumentMetadata::PlainText(PlainMetadata { title: "t".into() });
        assert_eq!(txt.slug(), None);

        let pdf = DocumentMetadata::Pdf(PdfMetadata {
            title: "t".into(),
            original_filename: None,
        });
        assert_eq!(pdf.slug(), None);
    }

    #[test]
    fn content_hash_constructor_preserves_string() {
        let h = ContentHash::new("deadbeef".into());
        assert_eq!(h.as_hex(), "deadbeef");
        assert_eq!(h.to_string(), "deadbeef");
    }

    #[test]
    fn pdf_metadata_original_filename_defaults_to_none() {
        let raw = r#"{"title": "t"}"#;
        let pdf: PdfMetadata = serde_json::from_str(raw).expect("parse");
        assert_eq!(pdf.title, "t");
        assert_eq!(pdf.original_filename, None);
    }
}
