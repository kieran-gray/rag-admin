use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::connector::ConnectorQueryService;
use crate::server::application::connector_import::ConnectorImportQueryService;
use crate::server::application::markdown::{Block, BlockKind};
use crate::server::application::ports::MarkdownParser;
use crate::server::application::source_document::ports::BlobStore;
use crate::server::application::AppError;
use crate::server::domain::chunk_set::repository::ChunkSetRepository;
use crate::server::domain::indexing::read_model::IndexingReadModel;
use crate::server::domain::indexing::repository::IndexingRepository;
use crate::server::domain::source_document::read_model::SourceDocumentReadModel;
use crate::server::domain::source_document::repository::{
    DocumentListCursor, DocumentListQuery, DocumentStatusFilter, SourceDocumentRepository,
    SourceFilter,
};
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::shared::contracts::{
    ChunkDto, ConnectorFacetDto, DocumentConnectorDto, DocumentListItemDto, DocumentListPageDto,
    DocumentListQueryDto, DocumentStatusFilterDto, IndexingDto, MarkdownBlockDto,
    MarkdownBlockKindDto, SourceDescriptorDto, SourceDocumentDetailDto, SourceDocumentDto,
    SourceDocumentMarkdownDto, SourceFacetDto, SourceFilterDto,
};

pub struct SourceDocumentQueryService {
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    indexing_repository: Arc<dyn IndexingRepository>,
    chunk_set_repository: Arc<dyn ChunkSetRepository>,
    blob_store: Arc<dyn BlobStore>,
    markdown_parser: Arc<dyn MarkdownParser>,
    connector_query_service: Arc<ConnectorQueryService>,
    connector_import_query_service: Arc<ConnectorImportQueryService>,
}

impl SourceDocumentQueryService {
    pub fn new(
        source_document_repository: Arc<dyn SourceDocumentRepository>,
        indexing_repository: Arc<dyn IndexingRepository>,
        chunk_set_repository: Arc<dyn ChunkSetRepository>,
        blob_store: Arc<dyn BlobStore>,
        markdown_parser: Arc<dyn MarkdownParser>,
        connector_query_service: Arc<ConnectorQueryService>,
        connector_import_query_service: Arc<ConnectorImportQueryService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source_document_repository,
            indexing_repository,
            chunk_set_repository,
            blob_store,
            markdown_parser,
            connector_query_service,
            connector_import_query_service,
        })
    }

    pub async fn list(&self) -> Result<Vec<SourceDocumentDto>, AppError> {
        let docs = self.source_document_repository.list().await?;
        Ok(docs.iter().map(map_doc_to_dto).collect())
    }

    pub async fn list_documents(&self) -> Result<Vec<DocumentListItemDto>, AppError> {
        let docs = self.source_document_repository.list().await?;
        let document_ids: Vec<Uuid> = docs.iter().map(|d| d.document_id).collect();

        let indexings = self
            .indexing_repository
            .list_for_documents(&document_ids)
            .await?;

        let mut indexings_by_doc: HashMap<Uuid, Vec<IndexingDto>> = HashMap::new();
        for indexing in &indexings {
            indexings_by_doc
                .entry(indexing.document_id)
                .or_default()
                .push(map_indexing_to_dto(indexing));
        }

        let mut connectors_by_doc = self.build_connectors_by_document(&document_ids).await?;

        Ok(docs
            .iter()
            .map(|doc| {
                let indexings = indexings_by_doc
                    .remove(&doc.document_id)
                    .unwrap_or_default();
                let connectors = connectors_by_doc
                    .remove(&doc.document_id)
                    .unwrap_or_default();
                let dto = map_doc_to_dto(doc);
                DocumentListItemDto {
                    source_ref_key: dto.source_ref_key,
                    document_type: dto.document_type,
                    title: dto.title,
                    document_id: Some(dto.document_id),
                    latest_version: Some(dto.latest_version),
                    latest_content_hash: Some(dto.latest_content_hash),
                    indexings,
                    source: source_descriptor_from(&doc.source_ref),
                    connectors,
                    updated_at: doc.latest_version_occurred_at.clone(),
                }
            })
            .collect())
    }

    async fn build_connectors_by_document(
        &self,
        document_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<DocumentConnectorDto>>, AppError> {
        let imports = self
            .connector_import_query_service
            .list_for_documents(document_ids)
            .await?;
        if imports.is_empty() {
            return Ok(HashMap::new());
        }
        let connectors = self.connector_query_service.list().await?;
        let name_by_id: HashMap<Uuid, String> = connectors
            .iter()
            .map(|c| (c.connector_id, c.name.clone()))
            .collect();
        let mut by_doc: HashMap<Uuid, Vec<DocumentConnectorDto>> = HashMap::new();
        for record in imports {
            let name = name_by_id
                .get(&record.connector_id)
                .cloned()
                .unwrap_or_else(|| "(removed)".to_string());
            by_doc
                .entry(record.document_id)
                .or_default()
                .push(DocumentConnectorDto {
                    connector_id: record.connector_id,
                    name,
                });
        }
        Ok(by_doc)
    }

    pub async fn list_documents_page(
        &self,
        query: DocumentListQueryDto,
    ) -> Result<DocumentListPageDto, AppError> {
        let document_id_filter = if query.connectors.is_empty() {
            None
        } else {
            Some(
                self.connector_import_query_service
                    .document_ids_for_connectors(&query.connectors)
                    .await?,
            )
        };

        let domain_query = DocumentListQuery {
            cursor: query.cursor.as_deref().map(decode_cursor).transpose()?,
            limit: if query.limit == 0 { 25 } else { query.limit },
            search: query.search.clone(),
            sources: query.sources.iter().map(source_filter_from_dto).collect(),
            statuses: query.statuses.iter().map(status_filter_from_dto).collect(),
            document_id_filter,
        };

        let page = self
            .source_document_repository
            .list_page(&domain_query)
            .await?;

        let document_ids: Vec<Uuid> = page.items.iter().map(|e| e.document.document_id).collect();
        let indexings = self
            .indexing_repository
            .list_for_documents(&document_ids)
            .await?;
        let mut indexings_by_doc: HashMap<Uuid, Vec<IndexingDto>> = HashMap::new();
        for indexing in &indexings {
            indexings_by_doc
                .entry(indexing.document_id)
                .or_default()
                .push(map_indexing_to_dto(indexing));
        }

        let mut connectors_by_doc = self.build_connectors_by_document(&document_ids).await?;

        let items: Vec<DocumentListItemDto> = page
            .items
            .iter()
            .map(|entry| {
                let doc = &entry.document;
                let indexings = indexings_by_doc
                    .remove(&doc.document_id)
                    .unwrap_or_default();
                let connectors = connectors_by_doc
                    .remove(&doc.document_id)
                    .unwrap_or_default();
                let dto = map_doc_to_dto(doc);
                DocumentListItemDto {
                    source_ref_key: dto.source_ref_key,
                    document_type: dto.document_type,
                    title: dto.title,
                    document_id: Some(dto.document_id),
                    latest_version: Some(dto.latest_version),
                    latest_content_hash: Some(dto.latest_content_hash),
                    indexings,
                    source: source_descriptor_from(&doc.source_ref),
                    connectors,
                    updated_at: entry.updated_at.clone(),
                }
            })
            .collect();

        let facets = self.source_document_repository.source_facets().await?;
        let sources = facets
            .into_iter()
            .map(|(filter, count)| SourceFacetDto {
                source: source_descriptor_from_filter(&filter),
                document_count: count,
            })
            .collect();

        let connector_facet_pairs = self.connector_import_query_service.facets().await?;
        let connector_dtos = self.connector_query_service.list().await?;
        let name_by_id: HashMap<Uuid, String> = connector_dtos
            .into_iter()
            .map(|c| (c.connector_id, c.name))
            .collect();
        let connectors: Vec<ConnectorFacetDto> = connector_facet_pairs
            .into_iter()
            .map(|(connector_id, count)| ConnectorFacetDto {
                connector_id,
                name: name_by_id
                    .get(&connector_id)
                    .cloned()
                    .unwrap_or_else(|| "(removed)".to_string()),
                document_count: count,
            })
            .collect();

        Ok(DocumentListPageDto {
            items,
            next_cursor: page.next_cursor.as_ref().map(encode_cursor),
            total_matching: page.total_matching,
            total_all: page.total_all,
            sources,
            connectors,
        })
    }

    pub async fn get_detail_by_source_ref(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentDetailDto>, AppError> {
        let doc = self
            .source_document_repository
            .find_by_source_ref(source_ref)
            .await?;
        match doc {
            None => Ok(None),
            Some(doc) => self.get_detail(doc.document_id).await,
        }
    }

    pub async fn get_detail(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SourceDocumentDetailDto>, AppError> {
        let doc = self.source_document_repository.load(document_id).await?;

        match doc {
            None => Ok(None),
            Some(doc) => {
                let indexings = self
                    .indexing_repository
                    .list_for_document(document_id)
                    .await?;

                Ok(Some(SourceDocumentDetailDto {
                    document: map_doc_to_dto(&doc),
                    indexings: indexings.iter().map(map_indexing_to_dto).collect(),
                }))
            }
        }
    }

    pub async fn get_source_markdown(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentMarkdownDto>, AppError> {
        let Some(doc) = self
            .source_document_repository
            .find_by_source_ref(source_ref)
            .await?
        else {
            return Ok(None);
        };

        let bytes = self.blob_store.get(&doc.latest_content_hash).await?;
        let source = String::from_utf8(bytes).map_err(|e| {
            AppError::Internal(format!("content for {} is not utf-8: {e}", doc.document_id))
        })?;

        let parsed = self.markdown_parser.parse(&source)?;
        let blocks = parsed.blocks.iter().map(block_to_dto).collect();

        let title = doc.latest_metadata.title().to_string();

        Ok(Some(SourceDocumentMarkdownDto {
            document_id: doc.document_id,
            source_ref_key: doc.source_ref.natural_key(),
            title,
            version: doc.latest_version_number,
            source,
            blocks,
        }))
    }

    pub async fn get_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<ChunkDto>, AppError> {
        let chunks = self.chunk_set_repository.load_chunks(chunk_set_id).await?;
        Ok(chunks
            .into_iter()
            .map(|c| ChunkDto {
                chunk_id: c.chunk_id,
                sequence: c.sequence,
                heading: c.heading,
                text: c.text,
                char_start: c.char_start,
                char_end: c.char_end,
            })
            .collect())
    }
}

fn block_to_dto(block: &Block) -> MarkdownBlockDto {
    let (kind, heading_depth) = match &block.kind {
        BlockKind::Heading(h) => (MarkdownBlockKindDto::Heading, Some(h.depth)),
        BlockKind::Paragraph => (MarkdownBlockKindDto::Paragraph, None),
        BlockKind::List => (MarkdownBlockKindDto::List, None),
        BlockKind::CodeFence => (MarkdownBlockKindDto::CodeFence, None),
        BlockKind::BlockQuote => (MarkdownBlockKindDto::BlockQuote, None),
        BlockKind::Table => (MarkdownBlockKindDto::Table, None),
        BlockKind::Html => (MarkdownBlockKindDto::Html, None),
        BlockKind::ThematicBreak => (MarkdownBlockKindDto::ThematicBreak, None),
        BlockKind::Other => (MarkdownBlockKindDto::Other, None),
    };
    MarkdownBlockDto {
        kind,
        html: markdown::to_html(&block.text),
        char_start: block.span.char_start as u32,
        char_end: block.span.char_end as u32,
        heading_depth,
    }
}

fn map_indexing_to_dto(i: &IndexingReadModel) -> IndexingDto {
    IndexingDto {
        indexing_id: i.indexing_id,
        pipeline_configuration_id: i.pipeline_configuration_id,
        document_version: i.document_version,
        status: format!("{:?}", i.status),
        attempts: i.attempts,
        chunk_set_id: i.chunk_set_id,
        embedding_set_id: i.embedding_set_id,
        removed: i.removed,
    }
}

fn source_descriptor_from(source_ref: &SourceRef) -> SourceDescriptorDto {
    match source_ref {
        SourceRef::Upload { .. } => SourceDescriptorDto::Upload,
        SourceRef::Url { url } => SourceDescriptorDto::Url {
            host: host_from_url(url).unwrap_or_else(|| "unknown".to_string()),
            url: url.clone(),
        },
    }
}

fn source_descriptor_from_filter(filter: &SourceFilter) -> SourceDescriptorDto {
    match filter {
        SourceFilter::Upload => SourceDescriptorDto::Upload,
        SourceFilter::UrlHost(host) => SourceDescriptorDto::Url {
            host: host.clone(),
            url: format!("https://{host}"),
        },
    }
}

fn source_filter_from_dto(dto: &SourceFilterDto) -> SourceFilter {
    match dto {
        SourceFilterDto::Upload => SourceFilter::Upload,
        SourceFilterDto::UrlHost { host } => SourceFilter::UrlHost(host.clone()),
    }
}

fn status_filter_from_dto(dto: &DocumentStatusFilterDto) -> DocumentStatusFilter {
    match dto {
        DocumentStatusFilterDto::Registered => DocumentStatusFilter::Registered,
        DocumentStatusFilterDto::InProgress => DocumentStatusFilter::InProgress,
        DocumentStatusFilterDto::Indexed => DocumentStatusFilter::Indexed,
        DocumentStatusFilterDto::Failed => DocumentStatusFilter::Failed,
    }
}

fn encode_cursor(cursor: &DocumentListCursor) -> String {
    format!("{}|{}", cursor.updated_at, cursor.document_id)
}

fn decode_cursor(value: &str) -> Result<DocumentListCursor, AppError> {
    let (updated_at, document_id_str) = value
        .rsplit_once('|')
        .ok_or_else(|| AppError::Validation("invalid cursor".to_string()))?;
    let document_id = Uuid::parse_str(document_id_str)
        .map_err(|e| AppError::Validation(format!("cursor uuid: {e}")))?;
    Ok(DocumentListCursor {
        updated_at: updated_at.to_string(),
        document_id,
    })
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

fn map_doc_to_dto(doc: &SourceDocumentReadModel) -> SourceDocumentDto {
    let title = doc.latest_metadata.title().to_string();
    SourceDocumentDto {
        document_id: doc.document_id,
        document_type: format!("{:?}", doc.document_type),
        source_ref_key: doc.source_ref.natural_key(),
        title,
        latest_version: doc.latest_version_number,
        latest_content_hash: doc.latest_content_hash.as_hex().to_string(),
        deleted: doc.deleted,
    }
}
