use leptos::prelude::*;

use crate::contracts::{
    ChunkDto, DocumentListItemDto, SourceDocumentDetailDto, SourceDocumentDto,
    SourceDocumentMarkdownDto,
};
use crate::core::ChunkingConfig;

#[cfg(feature = "ssr")]
use crate::server::application::source_document::{
    ports::SourceAdapterRegistry, SourceDocumentIngestService, SourceDocumentQueryService,
};
#[cfg(feature = "ssr")]
use crate::server::domain::source_document::{document_type::DocumentType, source_ref::SourceRef};
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = GetChunks, prefix = "/api", endpoint = "get_chunks")]
pub async fn get_chunks(chunk_set_id: uuid::Uuid) -> Result<Vec<ChunkDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .get_chunks(chunk_set_id)
        .await
        .map_err(map_app_error)
}

#[server(
    name = StartSourceDocumentIngest,
    prefix = "/api",
    endpoint = "start_source_document_ingest"
)]
pub async fn start_source_document_ingest(
    source_ref_slug: String,
    pipeline_configuration_id: uuid::Uuid,
    chunking_config: ChunkingConfig,
) -> Result<uuid::Uuid, ServerFnError> {
    let ingest = ctx::<Arc<SourceDocumentIngestService>>()?;

    ingest
        .import_document(
            SourceRef::UpstreamSlug {
                slug: source_ref_slug.clone(),
            },
            DocumentType::BlogPost,
        )
        .await
        .map_err(map_app_error)?;

    ingest
        .request_indexing(
            SourceRef::UpstreamSlug {
                slug: source_ref_slug,
            },
            pipeline_configuration_id,
            chunking_config,
            true,
        )
        .await
        .map_err(map_app_error)
}

#[server(
    name = StartIndexingWithDefaults,
    prefix = "/api",
    endpoint = "start_indexing_with_defaults"
)]
pub async fn start_indexing_with_defaults(
    source_ref_slug: String,
) -> Result<uuid::Uuid, ServerFnError> {
    use crate::server::application::configuration::{
        ChunkingConfigurationQueryService, PipelineConfigurationQueryService,
    };
    use crate::server::application::AppError;

    let ingest = ctx::<Arc<SourceDocumentIngestService>>()?;
    let pipeline_query = ctx::<Arc<PipelineConfigurationQueryService>>()?;
    let chunking_query = ctx::<Arc<ChunkingConfigurationQueryService>>()?;

    let pipeline = pipeline_query
        .list()
        .await
        .map_err(map_app_error)?
        .into_iter()
        .find(|p| p.is_default)
        .ok_or_else(|| {
            map_app_error(AppError::Validation(
                "no default pipeline configured".into(),
            ))
        })?;

    let chunking = chunking_query
        .list()
        .await
        .map_err(map_app_error)?
        .into_iter()
        .find(|c| c.is_default)
        .ok_or_else(|| {
            map_app_error(AppError::Validation(
                "no default chunking configuration".into(),
            ))
        })?;

    ingest
        .request_indexing(
            SourceRef::parse_route_key(&source_ref_slug),
            pipeline.pipeline_configuration_id,
            chunking.config,
            true,
        )
        .await
        .map_err(map_app_error)
}

#[server(
    name = ImportSourceDocument,
    prefix = "/api",
    endpoint = "import_source_document"
)]
pub async fn import_source_document(
    source_ref_slug: String,
) -> Result<SourceDocumentDto, ServerFnError> {
    ctx::<Arc<SourceDocumentIngestService>>()?
        .import_document(
            SourceRef::UpstreamSlug {
                slug: source_ref_slug,
            },
            DocumentType::BlogPost,
        )
        .await
        .map_err(map_app_error)
}

#[server(
    name = ImportSourceDocumentFromUrl,
    prefix = "/api",
    endpoint = "import_source_document_from_url"
)]
pub async fn import_source_document_from_url(
    url: String,
) -> Result<SourceDocumentDto, ServerFnError> {
    ctx::<Arc<SourceDocumentIngestService>>()?
        .import_url(url)
        .await
        .map_err(map_app_error)
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
    ctx::<Arc<SourceDocumentIngestService>>()?
        .request_indexing(
            SourceRef::UpstreamSlug {
                slug: source_ref_slug,
            },
            pipeline_configuration_id,
            chunking_config,
            auto_advance,
        )
        .await
        .map_err(map_app_error)
}

#[server(name = RequeueChunking, prefix = "/api", endpoint = "requeue_chunking")]
pub async fn requeue_chunking(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<SourceDocumentIngestService>>()?
        .requeue_chunking(indexing_id)
        .await
        .map_err(map_app_error)
}

#[server(name = RequeueEmbedding, prefix = "/api", endpoint = "requeue_embedding")]
pub async fn requeue_embedding(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<SourceDocumentIngestService>>()?
        .requeue_embedding(indexing_id)
        .await
        .map_err(map_app_error)
}

#[server(name = RequeueIndexing, prefix = "/api", endpoint = "requeue_indexing")]
pub async fn requeue_indexing(indexing_id: uuid::Uuid) -> Result<(), ServerFnError> {
    ctx::<Arc<SourceDocumentIngestService>>()?
        .requeue_indexing(indexing_id)
        .await
        .map_err(map_app_error)
}

#[server(
    name = ListDocuments,
    prefix = "/api",
    endpoint = "list_documents"
)]
pub async fn list_documents() -> Result<Vec<DocumentListItemDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .list_documents()
        .await
        .map_err(map_app_error)
}

#[server(
    name = ListAdapterDocuments,
    prefix = "/api",
    endpoint = "list_adapter_documents"
)]
pub async fn list_adapter_documents() -> Result<Vec<DocumentListItemDto>, ServerFnError> {
    let adapters = ctx::<Arc<SourceAdapterRegistry>>()?;
    let query = ctx::<Arc<SourceDocumentQueryService>>()?;

    let available = adapters.list_all().await.map_err(map_app_error)?;

    let existing: Vec<SourceDocumentDto> = query.list().await.map_err(map_app_error)?;

    let existing_map: std::collections::HashMap<String, SourceDocumentDto> = existing
        .into_iter()
        .map(|d| (d.source_ref_key.clone(), d))
        .collect();

    let mut items = vec![];

    for (doc_type, summary) in available {
        let key = summary.source_ref.natural_key();
        if !existing_map.contains_key(&key) {
            items.push(DocumentListItemDto {
                source_ref_key: key,
                document_type: doc_type,
                title: summary.title,
                document_id: None,
                latest_version: None,
                latest_content_hash: None,
                indexings: vec![],
            })
        }
    }

    Ok(items)
}

#[server(
    name = GetDocumentDetailBySourceRef,
    prefix = "/api",
    endpoint = "get_document_detail_by_source_ref"
)]
pub async fn get_document_detail_by_source_ref(
    source_ref_slug: String,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .get_detail_by_source_ref(&SourceRef::parse_route_key(&source_ref_slug))
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetDocumentDetailById,
    prefix = "/api",
    endpoint = "get_document_detail_by_id"
)]
pub async fn get_document_detail_by_id(
    document_id: uuid::Uuid,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .get_detail(document_id)
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetDocumentSource,
    prefix = "/api",
    endpoint = "get_document_source"
)]
pub async fn get_document_source(
    source_ref_slug: String,
) -> Result<Option<SourceDocumentMarkdownDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .get_source_markdown(&SourceRef::parse_route_key(&source_ref_slug))
        .await
        .map_err(map_app_error)
}

#[server(
    name = ListSourceDocuments,
    prefix = "/api",
    endpoint = "list_source_documents"
)]
pub async fn list_source_documents() -> Result<Vec<SourceDocumentDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .list()
        .await
        .map_err(map_app_error)
}

#[server(
    name = GetSourceDocumentDetail,
    prefix = "/api",
    endpoint = "get_source_document_detail"
)]
pub async fn get_source_document_detail(
    document_id: uuid::Uuid,
) -> Result<Option<SourceDocumentDetailDto>, ServerFnError> {
    ctx::<Arc<SourceDocumentQueryService>>()?
        .get_detail(document_id)
        .await
        .map_err(map_app_error)
}
