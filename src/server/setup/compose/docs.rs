use std::sync::Arc;

use crate::server::application::docs::GuidesQueryService;
use crate::server::application::ports::MarkdownParser;
use crate::server::infrastructure::docs::FsGuideRepository;
use crate::server::setup::exceptions::SetupError;

pub struct DocsServices {
    pub guides_query_service: Arc<GuidesQueryService>,
}

pub struct DocsDeps {
    pub guides_dir: String,
    pub markdown_parser: Arc<dyn MarkdownParser>,
}

impl DocsServices {
    pub fn build(deps: DocsDeps) -> Result<Self, SetupError> {
        let repository = Arc::new(FsGuideRepository::new(deps.guides_dir));
        let guides_query_service =
            Arc::new(GuidesQueryService::new(repository, deps.markdown_parser));
        Ok(Self {
            guides_query_service,
        })
    }
}
