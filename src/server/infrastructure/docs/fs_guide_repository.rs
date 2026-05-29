use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use crate::server::application::docs::{GuideRepository, RawGuide};
use crate::server::application::AppError;

pub struct FsGuideRepository {
    base_dir: PathBuf,
}

impl FsGuideRepository {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
}

#[async_trait]
impl GuideRepository for FsGuideRepository {
    async fn list(&self) -> Result<Vec<RawGuide>, AppError> {
        let mut entries = match fs::read_dir(&self.base_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                tracing::warn!("guides directory {} not found", self.base_dir.display());
                return Ok(Vec::new());
            }
            Err(e) => return Err(AppError::Internal(format!("read guides directory: {e}"))),
        };

        let mut guides = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::Internal(format!("read guides directory: {e}")))?
        {
            let path = entry.path();
            if !is_markdown(&path) {
                continue;
            }
            let Some(slug) = slug_from_path(&path) else {
                continue;
            };
            let content = fs::read_to_string(&path)
                .await
                .map_err(|e| AppError::Internal(format!("read guide {slug}: {e}")))?;
            guides.push(parse_guide(slug, &content));
        }
        Ok(guides)
    }

    async fn get(&self, slug: &str) -> Result<Option<RawGuide>, AppError> {
        if !is_safe_slug(slug) {
            return Ok(None);
        }
        let path = self.base_dir.join(format!("{slug}.md"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(parse_guide(slug.to_string(), &content))),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AppError::Internal(format!("read guide {slug}: {e}"))),
        }
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("md")
}

fn slug_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
}

fn is_safe_slug(slug: &str) -> bool {
    !slug.is_empty() && !slug.contains(['/', '\\']) && !slug.contains("..")
}

fn parse_guide(slug: String, content: &str) -> RawGuide {
    let mut title: Option<String> = None;
    let mut summary = String::new();
    let mut section = "Guides".to_string();
    let mut order = 0;

    let body = if let Some((front, rest)) = split_frontmatter(content) {
        for line in front.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "title" => title = Some(value.to_string()),
                "summary" => summary = value.to_string(),
                "section" => section = value.to_string(),
                "order" => order = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        rest
    } else {
        content.to_string()
    };

    RawGuide {
        title: title.unwrap_or_else(|| slug.clone()),
        slug,
        summary,
        section,
        order,
        body,
    }
}

fn split_frontmatter(content: &str) -> Option<(String, String)> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut front = String::new();
    for line in lines.by_ref() {
        if line == "---" {
            let body = lines.collect::<Vec<_>>().join("\n");
            return Some((front, body));
        }
        front.push_str(line);
        front.push('\n');
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let content = "---\ntitle: Getting started\nsummary: An intro\nsection: Basics\norder: 2\n---\n# Heading\n\nBody text.";
        let guide = parse_guide("getting-started".to_string(), content);

        assert_eq!(guide.slug, "getting-started");
        assert_eq!(guide.title, "Getting started");
        assert_eq!(guide.summary, "An intro");
        assert_eq!(guide.section, "Basics");
        assert_eq!(guide.order, 2);
        assert_eq!(guide.body, "# Heading\n\nBody text.");
    }

    #[test]
    fn falls_back_to_defaults_without_frontmatter() {
        let guide = parse_guide("notes".to_string(), "# Just a body");

        assert_eq!(guide.title, "notes");
        assert_eq!(guide.summary, "");
        assert_eq!(guide.section, "Guides");
        assert_eq!(guide.order, 0);
        assert_eq!(guide.body, "# Just a body");
    }

    #[test]
    fn ignores_unknown_keys_and_keeps_body_thematic_breaks() {
        let content = "---\ntitle: T\nunknown: x\n---\nintro\n\n---\n\nmore";
        let guide = parse_guide("t".to_string(), content);

        assert_eq!(guide.title, "T");
        assert_eq!(guide.body, "intro\n\n---\n\nmore");
    }

    #[test]
    fn rejects_unsafe_slugs() {
        assert!(!is_safe_slug(""));
        assert!(!is_safe_slug("../secret"));
        assert!(!is_safe_slug("nested/guide"));
        assert!(is_safe_slug("getting-started"));
    }
}
