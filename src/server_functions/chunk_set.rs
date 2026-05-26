use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{
    ChunkSetListPageDto, ChunkSetListQueryDto, ChunkSetSummaryDto, DeleteChunkSetRequestDto,
    DeleteUnusedChunkSetsRequestDto, DeleteUnusedChunkSetsResponseDto, SetChunkSetPinnedRequestDto,
};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::indexing::IndexingServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(
    name = GetChunkSetsPage,
    prefix = "/api",
    endpoint = "get_chunk_sets_page"
)]
pub async fn get_chunk_sets_page(
    query: ChunkSetListQueryDto,
) -> Result<ChunkSetListPageDto, ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .chunk_set_query_service
        .list_page(query)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetChunkSetsForDocument,
    prefix = "/api",
    endpoint = "get_chunk_sets_for_document"
)]
pub async fn get_chunk_sets_for_document(
    document_id: Uuid,
) -> Result<Vec<ChunkSetSummaryDto>, ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .chunk_set_query_service
        .list_for_document(document_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = SetChunkSetPinned,
    prefix = "/api",
    endpoint = "set_chunk_set_pinned"
)]
pub async fn set_chunk_set_pinned(
    request: SetChunkSetPinnedRequestDto,
) -> Result<(), ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .chunk_set_command_handler
        .set_pinned(request.chunk_set_id, request.pinned)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = DeleteChunkSet,
    prefix = "/api",
    endpoint = "delete_chunk_set"
)]
pub async fn delete_chunk_set(request: DeleteChunkSetRequestDto) -> Result<(), ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .chunk_set_command_handler
        .delete(request.chunk_set_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = DeleteUnusedChunkSets,
    prefix = "/api",
    endpoint = "delete_unused_chunk_sets"
)]
pub async fn delete_unused_chunk_sets(
    request: DeleteUnusedChunkSetsRequestDto,
) -> Result<DeleteUnusedChunkSetsResponseDto, ServerFnError> {
    let deleted = ctx::<Arc<IndexingServices>>()?
        .chunk_set_command_handler
        .delete_unused(request.older_than_seconds)
        .await
        .map_err(|e| map_app_error(&e))?;
    Ok(DeleteUnusedChunkSetsResponseDto { deleted })
}
