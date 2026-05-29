use serde::{Deserialize, Serialize};

use super::MarkdownBlockDto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideSummaryDto {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub section: String,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideSectionDto {
    pub label: String,
    pub guides: Vec<GuideSummaryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideDto {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub section: String,
    pub blocks: Vec<MarkdownBlockDto>,
}
