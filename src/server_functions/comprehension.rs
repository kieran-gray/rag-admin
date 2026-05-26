use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{
    DocumentMapDetailDto, DocumentMapSummaryDto, RequestMapBuildRequestDto,
    RequestMapBuildResponseDto,
};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::comprehension::ComprehensionServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(
    name = GetDocumentMap,
    prefix = "/api",
    endpoint = "get_document_map"
)]
pub async fn get_document_map(
    document_id: Uuid,
    document_version: u32,
) -> Result<Option<DocumentMapDetailDto>, ServerFnError> {
    ctx::<Arc<ComprehensionServices>>()?
        .comprehension_query_service
        .get_for_document(document_id, document_version)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ListDocumentMaps,
    prefix = "/api",
    endpoint = "list_document_maps"
)]
pub async fn list_document_maps(
    document_id: Uuid,
) -> Result<Vec<DocumentMapSummaryDto>, ServerFnError> {
    ctx::<Arc<ComprehensionServices>>()?
        .comprehension_query_service
        .list_for_document(document_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = RequestDocumentMapBuild,
    prefix = "/api",
    endpoint = "request_document_map_build"
)]
pub async fn request_document_map_build(
    request: RequestMapBuildRequestDto,
) -> Result<RequestMapBuildResponseDto, ServerFnError> {
    let handle = ctx::<Arc<ComprehensionServices>>()?
        .document_map_command_handler
        .request_build_for_document(
            request.document_id,
            request.document_version,
            request.generation_model_id,
        )
        .await
        .map_err(|e| map_app_error(&e))?;
    Ok(RequestMapBuildResponseDto {
        map_id: handle.map_id,
        chunk_set_id: handle.chunk_set_id,
        chunk_count: handle.chunk_count,
        status: handle.status.as_str().to_string(),
    })
}
