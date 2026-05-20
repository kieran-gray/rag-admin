use std::sync::Arc;

use crate::server::application::ports::ExtractedDocument;
use crate::server::application::source_document::ports::PdfToMarkdown;
use crate::server::application::AppError;

pub struct PdfExtractConverter;

impl PdfExtractConverter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl PdfToMarkdown for PdfExtractConverter {
    fn convert(&self, bytes: &[u8]) -> Result<ExtractedDocument, AppError> {
        let text = pdf_extract::extract_text_from_mem(bytes)
            .map_err(|e| AppError::Validation(format!("could not parse PDF: {e}")))?;
        let markdown = normalize_text(&text);
        if markdown.trim().is_empty() {
            return Err(AppError::Validation(
                "PDF has no extractable text; OCR is not supported in this version".into(),
            ));
        }
        Ok(ExtractedDocument {
            title: None,
            markdown,
        })
    }
}

fn normalize_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0usize;
    for line in raw.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out.trim_start_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_runs_of_blank_lines() {
        let raw = "first\n\n\n\nsecond\n\n\nthird\n";
        assert_eq!(normalize_text(raw), "first\n\nsecond\n\nthird\n");
    }

    #[test]
    fn trims_trailing_spaces_on_each_line() {
        let raw = "alpha   \nbeta\t\n";
        assert_eq!(normalize_text(raw), "alpha\nbeta\n");
    }
}
