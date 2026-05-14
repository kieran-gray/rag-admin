use leptos::prelude::*;

use crate::shared::{QueryRequest, QueryResult};

#[cfg(feature = "ssr")]
use crate::server::application::query::QueryService;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = QueryDocuments, prefix = "/api", endpoint = "query_documents")]
pub async fn query_documents(req: QueryRequest) -> Result<QueryResult, ServerFnError> {
    ctx::<Arc<QueryService>>()?
        .query(req)
        .await
        .map_err(map_app_error)
}
