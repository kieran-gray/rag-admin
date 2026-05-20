use leptos::prelude::*;

use crate::shared::contracts::{
    ChunkDto, DocumentListItemDto, DocumentListPageDto, DocumentListQueryDto,
    SourceDocumentDetailDto, SourceDocumentDto, SourceDocumentMarkdownDto,
};
use crate::shared::ChunkingConfig;

#[cfg(feature = "ssr")]
use crate::server::application::AppError;
#[cfg(feature = "ssr")]
use crate::server::domain::source_document::source_ref::SourceRef;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::indexing::IndexingServices;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::ingestion::IngestionServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
fn parse_source_ref(value: &str) -> Result<SourceRef, ServerFnError> {
    SourceRef::parse_route_key(value).ok_or_else(|| {
        map_app_error(&AppError::Validation(format!(
            "invalid source ref key: {value}"
        )))
    })
}

#[server(name = GetChunks, prefix = "/api", endpoint = "get_chunks")]
pub async fn get_chunks(chunk_set_id: uuid::Uuid) -> Result<Vec<ChunkDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .get_chunks(chunk_set_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = StartIndexingWithDefaults,
    prefix = "/api",
    endpoint = "start_indexing_with_defaults"
)]
pub async fn start_indexing_with_defaults(
    source_ref_slug: String,
) -> Result<uuid::Uuid, ServerFnError> {
    use crate::server::application::AppError;
    use crate::server::setup::compose::catalog::CatalogServices;

    let ingestion = ctx::<Arc<IngestionServices>>()?;
    let catalog = ctx::<Arc<CatalogServices>>()?;

    let pipeline = catalog
        .pipeline_configuration_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))?
        .into_iter()
        .find(|p| p.is_default)
        .ok_or_else(|| {
            map_app_error(&AppError::Validation(
                "no default pipeline configured".into(),
            ))
        })?;

    let chunking = catalog
        .chunking_configuration_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))?
        .into_iter()
        .find(|c| c.is_default)
        .ok_or_else(|| {
            map_app_error(&AppError::Validation(
                "no default chunking configuration".into(),
            ))
        })?;

    ingestion
        .source_document_ingest_service
        .request_indexing(
            parse_source_ref(&source_ref_slug)?,
            pipeline.pipeline_configuration_id,
            chunking.config,
            true,
        )
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ImportSourceDocumentFromUrl,
    prefix = "/api",
    endpoint = "import_source_document_from_url"
)]
pub async fn import_source_document_from_url(
    url: String,
) -> Result<SourceDocumentDto, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_ingest_service
        .import_url(url)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = RequestIndexing,
    prefix = "/api",
    endpoint = "request_indexing"
)]
pub async fn request_indexing(
    source_ref_slug: String,
    pipeline_configuration_id: uuid::Uuid,
    chunking_config: ChunkingConfig,
    auto_advance: bool,
) -> Result<uuid::Uuid, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_ingest_service
        .request_indexing(
            parse_source_ref(&source_ref_slug)?,
            pipeline_configuration_id,
            chunking_config,
            auto_advance,
        )
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = RequeueChunking, prefix = "/api", endpoint = "requeue_chunking")]
pub async fn requeue_chunking(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .indexing_command_handler
        .requeue_chunking(indexing_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = RequeueEmbedding, prefix = "/api", endpoint = "requeue_embedding")]
pub async fn requeue_embedding(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .indexing_command_handler
        .requeue_embedding(indexing_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = RequeueIndexing, prefix = "/api", endpoint = "requeue_indexing")]
pub async fn requeue_indexing(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<IndexingServices>>()?
        .indexing_command_handler
        .requeue_indexing(indexing_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ListDocuments,
    prefix = "/api",
    endpoint = "list_documents"
)]
pub async fn list_documents() -> Result<Vec<DocumentListItemDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .list_documents()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ListDocumentsPage,
    prefix = "/api",
    endpoint = "list_documents_page"
)]
pub async fn list_documents_page(
    query: DocumentListQueryDto,
) -> Result<DocumentListPageDto, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .list_documents_page(query)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = DeleteDocument,
    prefix = "/api",
    endpoint = "delete_document"
)]
pub async fn delete_document(document_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_command_handler
        .delete(document_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetDocumentDetailBySourceRef,
    prefix = "/api",
    endpoint = "get_document_detail_by_source_ref"
)]
pub async fn get_document_detail_by_source_ref(
    source_ref_slug: String,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .get_detail_by_source_ref(&parse_source_ref(&source_ref_slug)?)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetDocumentDetailById,
    prefix = "/api",
    endpoint = "get_document_detail_by_id"
)]
pub async fn get_document_detail_by_id(
    document_id: uuid::Uuid,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .get_detail(document_id)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetDocumentSource,
    prefix = "/api",
    endpoint = "get_document_source"
)]
pub async fn get_document_source(
    source_ref_slug: String,
) -> Result<Option<SourceDocumentMarkdownDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .get_source_markdown(&parse_source_ref(&source_ref_slug)?)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ListSourceDocuments,
    prefix = "/api",
    endpoint = "list_source_documents"
)]
pub async fn list_source_documents() -> Result<Vec<SourceDocumentDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = GetSourceDocumentDetail,
    prefix = "/api",
    endpoint = "get_source_document_detail"
)]
pub async fn get_source_document_detail(
    document_id: uuid::Uuid,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<IngestionServices>>()?
        .source_document_query_service
        .get_detail(document_id)
        .await
        .map_err(|e| map_app_error(&e))
}
