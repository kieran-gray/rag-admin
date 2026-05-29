use leptos::prelude::*;

use crate::shared::contracts::{GuideDto, GuideSectionDto};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::docs::DocsServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = ListGuides, prefix = "/api", endpoint = "list_guides")]
pub async fn list_guides() -> Result<Vec<GuideSectionDto>, ServerFnError> {
    ctx::<Arc<DocsServices>>()?
        .guides_query_service
        .list_sections()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = GetGuide, prefix = "/api", endpoint = "get_guide")]
pub async fn get_guide(slug: String) -> Result<Option<GuideDto>, ServerFnError> {
    ctx::<Arc<DocsServices>>()?
        .guides_query_service
        .get_guide(&slug)
        .await
        .map_err(|e| map_app_error(&e))
}
