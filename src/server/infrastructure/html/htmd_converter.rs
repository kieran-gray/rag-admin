use std::sync::Arc;

use crate::server::application::ports::html_to_markdown::{ExtractedDocument, HtmlToMarkdown};
use crate::server::application::AppError;

pub struct HtmdConverter;

impl HtmdConverter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl HtmlToMarkdown for HtmdConverter {
    fn convert(&self, html: &str) -> Result<ExtractedDocument, AppError> {
        let title = extract_title(html);
        let markdown = htmd::convert(html)
            .map_err(|e| AppError::Upstream(format!("html→markdown conversion failed: {e}")))?;
        Ok(ExtractedDocument { title, markdown })
    }
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let after_open = open + html.get(open..)?.find('>')?;
    let close = lower.get(after_open..)?.find("</title>")?;
    let raw = html.get(after_open + 1..after_open + close)?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(decode_entities(raw))
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
