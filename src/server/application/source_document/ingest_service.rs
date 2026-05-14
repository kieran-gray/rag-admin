use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::PipelineResolver;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::indexing::aggregate::Indexing;
use crate::server::domain::indexing::commands::{IndexingCommand, RequestIngest};
use crate::server::domain::source_document::commands::{
    AddVersion, CreateDocument, NewVersion, SourceDocumentCommand,
};
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::shared::{ChunkingConfig, SourceDocumentDto};

use super::{
    command_handler::SourceDocumentCommandHandler,
    ports::{BlobStore, SourceAdapterRegistry},
};
use crate::server::application::indexing::command_handler::IndexingCommandHandler;

pub struct SourceDocumentIngestServiceDeps {
    pub source_document_command_handler: Arc<SourceDocumentCommandHandler>,
    pub indexing_command_handler: Arc<IndexingCommandHandler>,
    pub source_document_repository: Arc<dyn SourceDocumentRepository>,
    pub blob_store: Arc<dyn BlobStore>,
    pub source_adapter_registry: Arc<SourceAdapterRegistry>,
    pub pipeline_resolver: Arc<PipelineResolver>,
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
            clock: deps.clock,
            id_generator: deps.id_generator,
        })
    }

    pub async fn import_document(
        &self,
        source_ref: SourceRef,
        document_type: DocumentType,
    ) -> Result<SourceDocumentDto, AppError> {
        let occurred_at = self.clock.now();
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
        let content_hash = self.blob_store.put(&fetched.content).await?;

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
                            metadata: fetched.metadata.clone(),
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
                                metadata: fetched.metadata.clone(),
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

        let title = match &fetched.metadata {
            crate::server::domain::source_document::version::DocumentMetadata::BlogPost(meta) => {
                meta.title.clone()
            }
        };

        Ok(SourceDocumentDto {
            document_id,
            document_type: format!("{document_type:?}"),
            source_ref_key: source_ref.natural_key().to_string(),
            title,
            latest_version: document_version,
            latest_content_hash: content_hash.as_hex().to_string(),
            deleted: false,
        })
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
