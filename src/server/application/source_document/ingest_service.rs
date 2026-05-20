use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::PipelineResolver;
use crate::server::application::connector::{ConnectorQueryService, ConnectorRegistry};
use crate::server::application::connector_import::ConnectorImportCommandHandler;
use crate::server::application::indexing::{
    command_handler::IndexingCommandHandler, RequestIngestInput,
};
use crate::server::application::ports::{Clock, HttpClient, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::connector::ConnectorConfig;
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::{ContentHash, DocumentMetadata};
use crate::shared::contracts::SourceDocumentDto;
use crate::shared::ChunkingConfig;

use super::ports::{AcquisitionHints, BlobStore, ContentType, NormalizedDocument, RawDocument};
use super::{command_handler::SourceDocumentCommandHandler, DocumentNormalizerRegistry};

pub struct SourceDocumentIngestServiceDeps {
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub source_document_repository: Arc<dyn SourceDocumentRepository>,
    pub blob_store: Arc<dyn BlobStore>,
    pub connector_registry: Arc<ConnectorRegistry>,
    pub connector_query_service: Arc<ConnectorQueryService>,
    pub connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
    pub pipeline_resolver: Arc<PipelineResolver>,
    pub http_client: Arc<dyn HttpClient>,
    pub normalizer_registry: Arc<DocumentNormalizerRegistry>,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
}

pub struct SourceDocumentIngestService {
    source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    indexing_command_handler: Arc<IndexingCommandHandler>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    connector_registry: Arc<ConnectorRegistry>,
    connector_query_service: Arc<ConnectorQueryService>,
    connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
    pipeline_resolver: Arc<PipelineResolver>,
    http_client: Arc<dyn HttpClient>,
    normalizer_registry: Arc<DocumentNormalizerRegistry>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl SourceDocumentIngestService {
    pub fn new(deps: SourceDocumentIngestServiceDeps) -> Arc<Self> {
        Arc::new(Self {
            source_document_command_handler: deps.source_document_command_handler,
            indexing_command_handler: deps.indexing_command_handler,
            source_document_repository: deps.source_document_repository,
            blob_store: deps.blob_store,
            connector_registry: deps.connector_registry,
            connector_query_service: deps.connector_query_service,
            connector_import_command_handler: deps.connector_import_command_handler,
            pipeline_resolver: deps.pipeline_resolver,
            http_client: deps.http_client,
            normalizer_registry: deps.normalizer_registry,
            clock: deps.clock,
            id_generator: deps.id_generator,
        })
    }

    pub async fn import_from_connector_with_sync(
        &self,
        connector_id: Uuid,
        source_ref: SourceRef,
        sync_id: Option<Uuid>,
    ) -> Result<SourceDocumentDto, AppError> {
        let connector = self.load_connector_config(connector_id).await?;
        let implementation = self.connector_registry.get(connector.kind())?;
        let raw = implementation.fetch(&connector, &source_ref).await?;
        let normalized = self.normalizer_registry.normalize(raw).await?;
        let source_ref_key = normalized.source_ref.natural_key();
        let dto = self.persist_document(normalized).await?;

        self.connector_import_command_handler
            .record(connector_id, dto.document_id, source_ref_key, sync_id)
            .await?;

        Ok(dto)
    }

    pub async fn import_upload(
        &self,
        bytes: Vec<u8>,
        filename: String,
    ) -> Result<SourceDocumentDto, AppError> {
        let content_type = ContentType::from_filename(&filename);
        let raw = RawDocument {
            source_ref: SourceRef::Upload {
                upload_id: self.id_generator.new_uuid(),
            },
            bytes,
            content_type,
            hints: AcquisitionHints {
                filename: Some(filename),
                ..AcquisitionHints::default()
            },
        };
        let normalized = self.normalizer_registry.normalize(raw).await?;
        self.persist_document(normalized).await
    }

    pub async fn import_url(&self, url: String) -> Result<SourceDocumentDto, AppError> {
        let url = url.trim().to_string();
        if url.is_empty() {
            return Err(AppError::Validation("url is empty".into()));
        }

        let (status, body) = self.http_client.get_text(&url).await?;
        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!("GET {url} returned {status}")));
        }

        let raw = RawDocument {
            source_ref: SourceRef::Url { url: url.clone() },
            bytes: body.into_bytes(),
            content_type: ContentType::Html,
            hints: AcquisitionHints {
                source_url: Some(url),
                fetched_at: Some(self.clock.now()),
                ..AcquisitionHints::default()
            },
        };
        let normalized = self.normalizer_registry.normalize(raw).await?;
        self.persist_document(normalized).await
    }

    async fn load_connector_config(&self, connector_id: Uuid) -> Result<ConnectorConfig, AppError> {
        self.connector_query_service
            .get_config(connector_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))
    }

    async fn persist_document(
        &self,
        normalized: NormalizedDocument,
    ) -> Result<SourceDocumentDto, AppError> {
        let NormalizedDocument {
            source_ref,
            document_type,
            content,
            metadata,
        } = normalized;

        let content_hash = self.blob_store.put(&content).await?;

        let existing = self
            .source_document_repository
            .find_by_source_ref(&source_ref)
            .await?;

        let (document_id, document_version) = match existing {
            None => {
                let document_id = self
                    .source_document_command_handler
                    .create_document(
                        document_type.clone(),
                        source_ref.clone(),
                        content_hash.clone(),
                        metadata.clone(),
                    )
                    .await?;
                (document_id, 1u32)
            }
            Some(existing_doc) => {
                if existing_doc.latest_content_hash == content_hash {
                    (existing_doc.document_id, existing_doc.latest_version_number)
                } else {
                    self.source_document_command_handler
                        .add_document_version(
                            existing_doc.document_id,
                            content_hash.clone(),
                            metadata.clone(),
                        )
                        .await?;
                    (
                        existing_doc.document_id,
                        existing_doc.latest_version_number + 1,
                    )
                }
            }
        };

        Ok(map_to_dto(
            document_id,
            &document_type,
            &source_ref,
            &metadata,
            document_version,
            &content_hash,
        ))
    }

    pub async fn request_indexing(
        &self,
        source_ref: SourceRef,
        pipeline_configuration_id: Uuid,
        chunking_config: ChunkingConfig,
        auto_advance: bool,
    ) -> Result<Uuid, AppError> {
        let document = self
            .source_document_repository
            .find_by_source_ref(&source_ref)
            .await?
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "document {} is not imported yet; call import_document first",
                    source_ref.natural_key()
                ))
            })?;

        let _ = self
            .pipeline_resolver
            .resolve(pipeline_configuration_id)
            .await?;

        self.indexing_command_handler
            .request_ingest(RequestIngestInput {
                document_id: document.document_id,
                pipeline_configuration_id,
                document_version: document.latest_version_number,
                chunking_config,
                auto_advance,
            })
            .await
    }
}

fn map_to_dto(
    document_id: Uuid,
    document_type: &DocumentType,
    source_ref: &SourceRef,
    metadata: &DocumentMetadata,
    document_version: u32,
    content_hash: &ContentHash,
) -> SourceDocumentDto {
    SourceDocumentDto {
        document_id,
        document_type: format!("{document_type:?}"),
        source_ref_key: source_ref.natural_key(),
        title: metadata.title().to_string(),
        latest_version: document_version,
        latest_content_hash: content_hash.as_hex().to_string(),
        deleted: false,
    }
}
