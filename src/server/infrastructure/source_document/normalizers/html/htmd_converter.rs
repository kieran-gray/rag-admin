use std::sync::Arc;

use readability_rust::Readability;

use crate::server::application::source_document::ports::html_to_markdown::{
    ExtractedDocument, HtmlToMarkdown,
};
use crate::server::application::AppError;

pub struct HtmdConverter;

impl HtmdConverter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl HtmlToMarkdown for HtmdConverter {
    fn convert(&self, html: &str) -> Result<ExtractedDocument, AppError> {
        let sanitized = strip_inline_scripts_and_styles(html);
        let (main_html, main_title) = extract_main_content(&sanitized);
        let title = main_title.or_else(|| extract_title(html));
        let markdown = htmd::convert(&main_html)
            .map_err(|e| AppError::Upstream(format!("html→markdown conversion failed: {e}")))?;
        Ok(ExtractedDocument { title, markdown })
    }
}

fn extract_main_content(html: &str) -> (String, Option<String>) {
    let Ok(mut readability) = Readability::new(html, None) else {
        return (html.to_string(), None);
    };
    let Some(extracted) = readability.parse() else {
        return (html.to_string(), None);
    };
    let body = extracted.content.unwrap_or_else(|| html.to_string());
    let title = extracted.title.filter(|t| !t.trim().is_empty());
    (body, title)
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

fn strip_inline_scripts_and_styles(html: &str) -> String {
    let no_scripts = strip_block(html.as_bytes(), b"<script", b"</script>");
    let no_styles = strip_block(&no_scripts, b"<style", b"</style>");
    String::from_utf8_lossy(&no_styles).into_owned()
}

fn strip_block(bytes: &[u8], open_prefix: &[u8], close_tag: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let tail = bytes.get(cursor..).unwrap_or_default();
        let Some(rel_open) = find_ci(tail, open_prefix) else {
            out.extend_from_slice(tail);
            break;
        };
        let abs_open_start = cursor + rel_open;
        out.extend_from_slice(bytes.get(cursor..abs_open_start).unwrap_or_default());

        let after_prefix = abs_open_start + open_prefix.len();
        let next_byte = bytes.get(after_prefix).copied();
        let is_tag_boundary = matches!(next_byte, Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/'));
        if !is_tag_boundary {
            out.extend_from_slice(bytes.get(abs_open_start..after_prefix).unwrap_or_default());
            cursor = after_prefix;
            continue;
        }

        let remainder = bytes.get(after_prefix..).unwrap_or_default();
        match find_ci(remainder, close_tag) {
            Some(rel_close) => cursor = after_prefix + rel_close + close_tag.len(),
            None => return out,
        }
    }

    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_block() {
        let html = "<p>hi</p><script>var x = 1;</script><p>bye</p>";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<p>hi</p><p>bye</p>");
    }

    #[test]
    fn strips_style_block_with_attributes() {
        let html = "<p>a</p><style media=\"screen\">body{color:red}</style><p>b</p>";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<p>a</p><p>b</p>");
    }

    #[test]
    fn strips_mixed_case() {
        let html = "<P>a</P><Script>code</Script><STYLE>x</STYLE>";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<P>a</P>");
    }

    #[test]
    fn leaves_noscript_alone() {
        let html = "<noscript>fallback</noscript>";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<noscript>fallback</noscript>");
    }

    #[test]
    fn strips_multiple_blocks() {
        let html = "<style>a</style><p>x</p><script>b</script><p>y</p><script>c</script>";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<p>x</p><p>y</p>");
    }

    #[test]
    fn drops_unterminated_script() {
        let html = "<p>before</p><script>oops";
        let cleaned = strip_inline_scripts_and_styles(html);
        assert_eq!(cleaned, "<p>before</p>");
    }

    #[test]
    fn readability_strips_chrome_and_keeps_main_content() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Example Site</title></head>
            <body>
                <nav>
                    <a href="/">Home</a>
                    <a href="/section-a">Section A</a>
                    <a href="/section-b">Section B</a>
                </nav>
                <header>
                    <h1>Example Site Header</h1>
                </header>
                <main>
                    <h1>The Real Content Heading</h1>
                    <p>This is the first paragraph of the main content. It contains enough text that
                    readability should clearly identify it as the primary content of the page,
                    distinct from the navigation chrome and footer boilerplate that surround it.</p>
                    <p>A second paragraph also containing meaningful prose so that readability's
                    text-density heuristics have something substantial to anchor on when choosing
                    which subtree of the document to extract as the main body.</p>
                    <h2>A Section</h2>
                    <p>More body text under a subheading, again with enough length that the
                    extraction algorithm will treat this region as load-bearing rather than as
                    boilerplate to discard.</p>
                </main>
                <footer>
                    <p>© 2026 Example Site</p>
                    <a href="/about">About</a>
                </footer>
            </body>
            </html>
        "#;

        let converter = HtmdConverter::new();
        let extracted = converter.convert(html).expect("convert");

        assert!(
            extracted.markdown.contains("The Real Content Heading"),
            "main heading should be kept: {}",
            extracted.markdown
        );
        assert!(
            extracted
                .markdown
                .contains("first paragraph of the main content"),
            "main body should be kept"
        );
        assert!(
            !extracted.markdown.contains("Example Site Header"),
            "site header should be dropped: {}",
            extracted.markdown
        );
        assert!(
            !extracted.markdown.contains("Section B"),
            "nav links should be dropped"
        );
        assert!(
            !extracted.markdown.contains("© 2026"),
            "footer should be dropped"
        );
    }
}
