use std::sync::Arc;

use crate::server::application::markdown::document_to_dto_blocks;
use crate::server::application::ports::MarkdownParser;
use crate::server::application::AppError;
use crate::shared::contracts::{GuideDto, GuideSectionDto, GuideSummaryDto};

use super::model::RawGuide;
use super::ports::GuideRepository;

pub struct GuidesQueryService {
    repository: Arc<dyn GuideRepository>,
    markdown_parser: Arc<dyn MarkdownParser>,
}

impl GuidesQueryService {
    pub fn new(
        repository: Arc<dyn GuideRepository>,
        markdown_parser: Arc<dyn MarkdownParser>,
    ) -> Self {
        Self {
            repository,
            markdown_parser,
        }
    }

    pub async fn list_sections(&self) -> Result<Vec<GuideSectionDto>, AppError> {
        let mut guides = self.repository.list().await?;
        guides.sort_by(|a, b| {
            a.section
                .cmp(&b.section)
                .then(a.order.cmp(&b.order))
                .then(a.title.cmp(&b.title))
        });

        let mut sections: Vec<GuideSectionDto> = Vec::new();
        for guide in guides {
            let summary = summary_dto(&guide);
            match sections.last_mut() {
                Some(section) if section.label == guide.section => section.guides.push(summary),
                _ => sections.push(GuideSectionDto {
                    label: guide.section,
                    guides: vec![summary],
                }),
            }
        }
        Ok(sections)
    }

    pub async fn get_guide(&self, slug: &str) -> Result<Option<GuideDto>, AppError> {
        let Some(guide) = self.repository.get(slug).await? else {
            return Ok(None);
        };
        let parsed = self.markdown_parser.parse(&guide.body)?;
        Ok(Some(GuideDto {
            slug: guide.slug,
            title: guide.title,
            summary: guide.summary,
            section: guide.section,
            blocks: document_to_dto_blocks(&parsed),
        }))
    }
}

fn summary_dto(guide: &RawGuide) -> GuideSummaryDto {
    GuideSummaryDto {
        slug: guide.slug.clone(),
        title: guide.title.clone(),
        summary: guide.summary.clone(),
        section: guide.section.clone(),
        order: guide.order,
    }
}
