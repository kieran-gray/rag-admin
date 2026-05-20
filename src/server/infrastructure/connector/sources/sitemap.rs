use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::str::from_utf8;
use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::connector::ports::{ConnectorImpl, DiscoveredItem};
use crate::server::application::ports::HttpClient;
use crate::server::application::source_document::ports::{
    AcquisitionHints, ContentType, RawDocument,
};
use crate::server::application::AppError;
use crate::server::domain::connector::{ConnectorConfig, ConnectorKind, SitemapConfig};
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::humanize_url;

const MAX_SITEMAP_DEPTH: usize = 4;

pub struct SitemapConnector {
    http: Arc<dyn HttpClient>,
}

impl SitemapConnector {
    pub fn new(http: Arc<dyn HttpClient>) -> Arc<Self> {
        Arc::new(Self { http })
    }

    async fn collect_urls(&self, config: &SitemapConfig) -> Result<Vec<String>, AppError> {
        let mut urls = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        self.walk_sitemap(&config.url, 0, &mut urls, &mut seen)
            .await?;
        Ok(urls
            .into_iter()
            .filter(|u| matches_patterns(u, &config.include_patterns, &config.exclude_patterns))
            .collect())
    }

    fn walk_sitemap<'a>(
        &'a self,
        url: &'a str,
        depth: usize,
        urls: &'a mut Vec<String>,
        seen: &'a mut HashSet<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            if depth > MAX_SITEMAP_DEPTH {
                return Ok(());
            }
            if !seen.insert(url.to_string()) {
                return Ok(());
            }

            let (status, body) = self.http.get_text(url).await?;
            if !(200..300).contains(&status) {
                return Err(AppError::Upstream(format!(
                    "sitemap GET {url} returned {status}"
                )));
            }

            let parsed = parse_sitemap(&body);
            match parsed {
                ParsedSitemap::Urls(items) => {
                    urls.extend(items);
                }
                ParsedSitemap::Index(sitemaps) => {
                    for child in sitemaps {
                        self.walk_sitemap(&child, depth + 1, urls, seen).await?;
                    }
                }
                ParsedSitemap::Empty => {}
            }
            Ok(())
        })
    }
}

#[async_trait]
impl ConnectorImpl for SitemapConnector {
    fn kind(&self) -> ConnectorKind {
        ConnectorKind::Sitemap
    }

    async fn list(&self, config: &ConnectorConfig) -> Result<Vec<DiscoveredItem>, AppError> {
        let ConnectorConfig::Sitemap(cfg) = config;
        let urls = self.collect_urls(cfg).await?;
        Ok(urls
            .into_iter()
            .map(|url| DiscoveredItem {
                title: humanize_url(&url),
                source_ref: SourceRef::Url { url },
            })
            .collect())
    }

    async fn fetch(
        &self,
        config: &ConnectorConfig,
        source_ref: &SourceRef,
    ) -> Result<RawDocument, AppError> {
        let ConnectorConfig::Sitemap(_) = config;
        let SourceRef::Url { url } = source_ref else {
            return Err(AppError::Validation(format!(
                "sitemap connector expects SourceRef::Url; got {source_ref:?}"
            )));
        };

        let (status, body) = self.http.get_text(url).await?;
        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!("GET {url} returned {status}")));
        }

        Ok(RawDocument {
            source_ref: source_ref.clone(),
            bytes: body.into_bytes(),
            content_type: ContentType::Html,
            hints: AcquisitionHints {
                source_url: Some(url.clone()),
                ..AcquisitionHints::default()
            },
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ParsedSitemap {
    Urls(Vec<String>),
    Index(Vec<String>),
    Empty,
}

fn parse_sitemap(xml: &str) -> ParsedSitemap {
    let lower = xml.to_ascii_lowercase();
    let is_index = lower.contains("<sitemapindex");
    let locs = extract_locs(xml);

    if locs.is_empty() {
        ParsedSitemap::Empty
    } else if is_index {
        ParsedSitemap::Index(locs)
    } else {
        ParsedSitemap::Urls(locs)
    }
}

fn extract_locs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = xml.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let Some(tail) = bytes.get(cursor..) else {
            break;
        };
        let Some(rel) = find_ci(tail, b"<loc") else {
            break;
        };
        let open_start = cursor + rel;
        let after_open_tag = open_start + 4;
        let remainder = bytes.get(after_open_tag..).unwrap_or_default();
        let Some(gt) = remainder.iter().position(|b| *b == b'>') else {
            break;
        };
        let content_start = after_open_tag + gt + 1;
        let after_open = bytes.get(content_start..).unwrap_or_default();
        let Some(close_rel) = find_ci(after_open, b"</loc>") else {
            break;
        };
        let content_end = content_start + close_rel;
        if let Some(content_bytes) = bytes.get(content_start..content_end) {
            if let Ok(text) = from_utf8(content_bytes) {
                let trimmed = decode_entities(text.trim());
                if !trimmed.is_empty() {
                    out.push(trimmed);
                }
            }
        }
        cursor = content_end + "</loc>".len();
    }

    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn find_ci(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return needle.is_empty().then_some(0);
    }
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle.iter())
            .all(|(h, n)| h.eq_ignore_ascii_case(n))
    })
}

fn matches_patterns(url: &str, include: &[String], exclude: &[String]) -> bool {
    if exclude.iter().any(|p| matches_pattern(url, p)) {
        return false;
    }
    if include.is_empty() {
        return true;
    }
    include.iter().any(|p| matches_pattern(url, p))
}

fn matches_pattern(url: &str, pattern: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if let Some(stripped) = pattern.strip_prefix('*') {
        if let Some(stripped) = stripped.strip_suffix('*') {
            return url.contains(stripped);
        }
        return url.ends_with(stripped);
    }
    if let Some(stripped) = pattern.strip_suffix('*') {
        return url.starts_with(stripped);
    }
    url.contains(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url><loc>https://example.com/a</loc></url>
                <url><loc>https://example.com/b</loc></url>
            </urlset>"#;
        let parsed = parse_sitemap(xml);
        assert_eq!(
            parsed,
            ParsedSitemap::Urls(vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string(),
            ])
        );
    }

    #[test]
    fn parses_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <sitemapindex>
                <sitemap><loc>https://example.com/sitemap-1.xml</loc></sitemap>
                <sitemap><loc>https://example.com/sitemap-2.xml</loc></sitemap>
            </sitemapindex>"#;
        let parsed = parse_sitemap(xml);
        assert_eq!(
            parsed,
            ParsedSitemap::Index(vec![
                "https://example.com/sitemap-1.xml".to_string(),
                "https://example.com/sitemap-2.xml".to_string(),
            ])
        );
    }

    #[test]
    fn decodes_html_entities_in_loc() {
        let xml = "<urlset><url><loc>https://example.com/?a=1&amp;b=2</loc></url></urlset>";
        let parsed = parse_sitemap(xml);
        assert_eq!(
            parsed,
            ParsedSitemap::Urls(vec!["https://example.com/?a=1&b=2".to_string()])
        );
    }

    #[test]
    fn include_patterns_filter() {
        assert!(matches_patterns(
            "https://example.com/blog/post",
            &["/blog/".into()],
            &[]
        ));
        assert!(!matches_patterns(
            "https://example.com/about",
            &["/blog/".into()],
            &[]
        ));
    }

    #[test]
    fn exclude_overrides_include() {
        assert!(!matches_patterns(
            "https://example.com/blog/draft",
            &["/blog/".into()],
            &["/draft".into()],
        ));
    }

    #[test]
    fn glob_prefix_pattern() {
        assert!(matches_pattern(
            "https://example.com/a/b",
            "https://example.com/*"
        ));
        assert!(!matches_pattern(
            "https://other.com/a",
            "https://example.com/*"
        ));
    }
}
