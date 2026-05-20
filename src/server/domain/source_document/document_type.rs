use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    Markdown,
    PlainText,
    WebPage,
    Pdf,
}

impl DocumentType {
    pub fn display_label(&self) -> &'static str {
        match self {
            DocumentType::Markdown => "Markdown",
            DocumentType::PlainText => "Plain text",
            DocumentType::WebPage => "Web page",
            DocumentType::Pdf => "PDF",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_label_covers_every_variant() {
        assert_eq!(DocumentType::Markdown.display_label(), "Markdown");
        assert_eq!(DocumentType::PlainText.display_label(), "Plain text");
        assert_eq!(DocumentType::WebPage.display_label(), "Web page");
        assert_eq!(DocumentType::Pdf.display_label(), "PDF");
    }

    #[test]
    fn serde_tagless_roundtrip() {
        for variant in [
            DocumentType::Markdown,
            DocumentType::PlainText,
            DocumentType::WebPage,
            DocumentType::Pdf,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: DocumentType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back);
        }
    }
}
