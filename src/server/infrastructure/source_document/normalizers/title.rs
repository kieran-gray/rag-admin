use std::path::Path;

use crate::server::application::source_document::ports::RawDocument;

pub fn title_from_hints(raw: &RawDocument) -> String {
    if let Some(t) = raw.hints.original_title.as_ref() {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(filename) = raw.hints.filename.as_ref() {
        if let Some(stem) = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return stem.to_string();
        }
        return filename.clone();
    }
    "Untitled".to_string()
}
