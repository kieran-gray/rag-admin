use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    Html,
    Markdown,
    PlainText,
    Pdf,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Html => "html",
            ContentType::Markdown => "markdown",
            ContentType::PlainText => "plain_text",
            ContentType::Pdf => "pdf",
        }
    }

    pub fn from_filename(filename: &str) -> Self {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        match ext.as_str() {
            "md" | "markdown" => ContentType::Markdown,
            "pdf" => ContentType::Pdf,
            "html" | "htm" => ContentType::Html,
            _ => ContentType::PlainText,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_extensions() {
        assert_eq!(ContentType::from_filename("notes.md"), ContentType::Markdown);
        assert_eq!(
            ContentType::from_filename("paper.pdf"),
            ContentType::Pdf
        );
        assert_eq!(ContentType::from_filename("page.html"), ContentType::Html);
        assert_eq!(ContentType::from_filename("page.htm"), ContentType::Html);
        assert_eq!(
            ContentType::from_filename("README.MARKDOWN"),
            ContentType::Markdown
        );
    }

    #[test]
    fn unknown_extension_falls_back_to_plain_text() {
        assert_eq!(
            ContentType::from_filename("notes.unknown"),
            ContentType::PlainText
        );
        assert_eq!(ContentType::from_filename("notes"), ContentType::PlainText);
    }
}
