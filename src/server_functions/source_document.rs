use leptos::prelude::*;

use crate::shared::{
    ChunkDto, ChunkingConfig, DocumentListItemDto, SourceDocumentDetailDto, SourceDocumentDto,
    SourceDocumentMarkdownDto,
};

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
    name = ListDocumentsWithStatus,
    prefix = "/api",
    endpoint = "list_documents_with_status"
)]
pub async fn list_documents_with_status() -> Result<Vec<DocumentListItemDto>, ServerFnError> {
    let adapters = ctx::<Arc<SourceAdapterRegistry>>()?;
    let query = ctx::<Arc<SourceDocumentQueryService>>()?;

    let available = adapters.list_all().await.map_err(map_app_error)?;

    let existing: Vec<SourceDocumentDto> = query.list().await.map_err(map_app_error)?;

    let mut existing_map: std::collections::HashMap<String, SourceDocumentDto> = existing
        .into_iter()
        .map(|d| (d.source_ref_key.clone(), d))
        .collect();

    let mut items: Vec<DocumentListItemDto> = available
        .into_iter()
        .map(|(doc_type, summary)| {
            let key = summary.source_ref.natural_key().to_string();
            match existing_map.remove(&key) {
                Some(existing_doc) => DocumentListItemDto {
                    source_ref_key: key,
                    document_type: doc_type,
                    title: summary.title,
                    document_id: Some(existing_doc.document_id),
                    latest_version: Some(existing_doc.latest_version),
                    latest_content_hash: Some(existing_doc.latest_content_hash),
                    indexings: vec![],
                },
                None => DocumentListItemDto {
                    source_ref_key: key,
                    document_type: doc_type,
                    title: summary.title,
                    document_id: None,
                    latest_version: None,
                    latest_content_hash: None,
                    indexings: vec![],
                },
            }
        })
        .collect();

    for leftover in existing_map.into_values() {
        items.push(DocumentListItemDto {
            source_ref_key: leftover.source_ref_key.clone(),
            document_type: leftover.document_type,
            title: leftover.title,
            document_id: Some(leftover.document_id),
            latest_version: Some(leftover.latest_version),
            latest_content_hash: Some(leftover.latest_content_hash),
            indexings: vec![],
        });
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
        .get_detail_by_source_ref(&SourceRef::UpstreamSlug {
            slug: source_ref_slug,
        })
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
        .get_source_markdown(&SourceRef::UpstreamSlug {
            slug: source_ref_slug,
        })
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
