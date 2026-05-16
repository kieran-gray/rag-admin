use std::path::Path;
use std::sync::Arc;

use uuid::Uuid;

use crate::contracts::SourceDocumentDto;
use crate::core::ChunkingConfig;
use crate::server::application::configuration::PipelineResolver;
use crate::server::application::ports::{Clock, HtmlToMarkdown, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::commands::{IndexingCommand, RequestIngest};
use crate::server::domain::source_document::commands::{
    AddVersion, CreateDocument, NewVersion, SourceDocumentCommand,
};
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::{
    ContentHash, DocumentMetadata, PlainMetadata, WebPageMetadata,
};
use crate::server::infrastructure::http_client::ReqwestHttpClient;

use super::{
    command_handler::SourceDocumentCommandHandler,
    ports::{BlobStore, SourceAdapterRegistry},
};
use crate::server::application::indexing::command_handler::IndexingCommandHandler;
use reqwest::header::HeaderMap;
use reqwest::Method;

pub struct SourceDocumentIngestServiceDeps {
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub source_document_repository: Arc<dyn SourceDocumentRepository>,
    pub blob_store: Arc<dyn BlobStore>,
    pub source_adapter_registry: Arc<SourceAdapterRegistry>,
    pub pipeline_resolver: Arc<PipelineResolver>,
    pub http_client: Arc<ReqwestHttpClient>,
    pub html_to_markdown: Arc<dyn HtmlToMarkdown>,
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
}

pub struct SourceDocumentIngestService {
    source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    indexing_command_handler: Arc<IndexingCommandHandler>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
    blob_store: Arc<dyn BlobStore>,
    source_adapter_registry: Arc<SourceAdapterRegistry>,
    pipeline_resolver: Arc<PipelineResolver>,
    http_client: Arc<ReqwestHttpClient>,
    html_to_markdown: Arc<dyn HtmlToMarkdown>,
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
            source_adapter_registry: deps.source_adapter_registry,
            pipeline_resolver: deps.pipeline_resolver,
            http_client: deps.http_client,
            html_to_markdown: deps.html_to_markdown,
            clock: deps.clock,
            id_generator: deps.id_generator,
        })
    }

    pub async fn import_document(
        &self,
        source_ref: SourceRef,
        document_type: DocumentType,
    ) -> Result<SourceDocumentDto, AppError> {
        let adapter = self
            .source_adapter_registry
            .get(&document_type)
            .ok_or_else(|| {
                AppError::Validation(format!("no adapter registered for {document_type:?}"))
            })?;
        let fetched = adapter
            .fetch(&source_ref)
            .await
            .map_err(|e| AppError::Upstream(format!("fetch failed: {e}")))?;

        self.persist_document(source_ref, document_type, fetched.content, fetched.metadata)
            .await
    }

    pub async fn import_upload(
        &self,
        bytes: Vec<u8>,
        filename: String,
    ) -> Result<SourceDocumentDto, AppError> {
        let document_type = document_type_from_filename(&filename);
        let title = derive_title_from_filename(&filename);
        let metadata = match document_type {
            DocumentType::Markdown => DocumentMetadata::Markdown(PlainMetadata { title }),
            DocumentType::PlainText => DocumentMetadata::PlainText(PlainMetadata { title }),
            DocumentType::BlogPost | DocumentType::WebPage => {
                return Err(AppError::Validation(format!(
                    "uploads cannot create {document_type:?}; supported types are Markdown and PlainText"
                )));
            }
        };

        let source_ref = SourceRef::Upload {
            upload_id: self.id_generator.new_uuid(),
        };
        self.persist_document(source_ref, document_type, bytes, metadata)
            .await
    }

    pub async fn import_url(&self, url: String) -> Result<SourceDocumentDto, AppError> {
        let url = url.trim().to_string();
        if url.is_empty() {
            return Err(AppError::Validation("url is empty".into()));
        }

        let (status, body) = self
            .http_client
            .request_text(Method::GET, &url, HeaderMap::new(), None)
            .await?;
        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!("GET {url} returned {status}")));
        }

        let extracted = self.html_to_markdown.convert(&body)?;
        let title = extracted.title.unwrap_or_else(|| url.clone());

        let metadata = DocumentMetadata::WebPage(WebPageMetadata {
            title,
            source_url: url.clone(),
            fetched_at: self.clock.now(),
        });

        self.persist_document(
            SourceRef::Url { url },
            DocumentType::WebPage,
            extracted.markdown.into_bytes(),
            metadata,
        )
        .await
    }

    async fn persist_document(
        &self,
        source_ref: SourceRef,
        document_type: DocumentType,
        content: Vec<u8>,
        metadata: DocumentMetadata,
    ) -> Result<SourceDocumentDto, AppError> {
        let occurred_at = self.clock.now();
        let content_hash = self.blob_store.put(&content).await?;

        let existing = self
            .source_document_repository
            .find_by_source_ref(&source_ref)
            .await?;

        let (document_id, document_version) = match existing {
            None => {
                let document_id = self.id_generator.new_uuid();
                self.source_document_command_handler
                    .handle(SourceDocumentCommand::CreateDocument(CreateDocument {
                        document_id,
                        document_type: document_type.clone(),
                        source_ref: source_ref.clone(),
                        initial_version: NewVersion {
                            content_hash: content_hash.clone(),
                            metadata: metadata.clone(),
                        },
                        occurred_at: occurred_at.clone(),
                    }))
                    .await?;
                (document_id, 1u32)
            }
            Some(existing_doc) => {
                if existing_doc.latest_content_hash == content_hash {
                    (existing_doc.document_id, existing_doc.latest_version_number)
                } else {
                    self.source_document_command_handler
                        .handle(SourceDocumentCommand::AddVersion(AddVersion {
                            document_id: existing_doc.document_id,
                            version: NewVersion {
                                content_hash: content_hash.clone(),
                                metadata: metadata.clone(),
                            },
                            occurred_at: occurred_at.clone(),
                        }))
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
            document_type,
            source_ref,
            metadata,
            document_version,
            content_hash,
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

        let occurred_at = self.clock.now();
        let request_id = self.id_generator.new_uuid();
        let indexing_id = Indexing::compute_id(document.document_id, pipeline_configuration_id);

        self.indexing_command_handler
            .handle(IndexingCommand::RequestIngest(RequestIngest {
                document_id: document.document_id,
                pipeline_configuration_id,
                document_version: document.latest_version_number,
                chunking_config,
                request_id,
                auto_advance,
                occurred_at,
            }))
            .await?;

        Ok(indexing_id)
    }

    pub async fn requeue_chunking(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.indexing_command_handler
            .handle_for(
                indexing_id,
                IndexingCommand::RequeueChunking(
                    crate::server::domain::indexing::commands::RequeueChunking {
                        occurred_at: self.clock.now(),
                    },
                ),
            )
            .await
    }

    pub async fn requeue_embedding(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.indexing_command_handler
            .handle_for(
                indexing_id,
                IndexingCommand::RequeueEmbedding(
                    crate::server::domain::indexing::commands::RequeueEmbedding {
                        occurred_at: self.clock.now(),
                    },
                ),
            )
            .await
    }

    pub async fn requeue_indexing(&self, indexing_id: Uuid) -> Result<(), AppError> {
        self.indexing_command_handler
            .handle_for(
                indexing_id,
                IndexingCommand::RequeueIndexing(
                    crate::server::domain::indexing::commands::RequeueIndexing {
                        occurred_at: self.clock.now(),
                    },
                ),
            )
            .await
    }
}

fn document_type_from_filename(filename: &str) -> DocumentType {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => DocumentType::Markdown,
        _ => DocumentType::PlainText,
    }
}

fn derive_title_from_filename(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| filename.to_string())
}

fn map_to_dto(
    document_id: Uuid,
    document_type: DocumentType,
    source_ref: SourceRef,
    metadata: DocumentMetadata,
    document_version: u32,
    content_hash: ContentHash,
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
