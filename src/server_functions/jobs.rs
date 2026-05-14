use leptos::prelude::*;

use crate::shared::ActivityJobDto;

#[cfg(feature = "ssr")]
use crate::server::application::ActivityRegistry;
#[cfg(feature = "ssr")]
use crate::server_functions::error::ctx;
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = ListActiveJobs, prefix = "/api", endpoint = "list_active_jobs")]
pub async fn list_active_jobs() -> Result<Vec<ActivityJobDto>, ServerFnError> {
    Ok(ctx::<Arc<ActivityRegistry>>()?.snapshot().await)
}
