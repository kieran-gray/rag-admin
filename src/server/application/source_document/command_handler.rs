use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::source_document::aggregate::SourceDocument;
use crate::server::domain::source_document::commands::{
    AddVersion, CreateDocument, DeleteDocument, NewVersion, SourceDocumentCommand,
};
use crate::server::domain::source_document::document_type::DocumentType;
use crate::server::domain::source_document::source_ref::SourceRef;
use crate::server::domain::source_document::version::{ContentHash, DocumentMetadata};
use event_sourcing::CommandProcessor;

pub struct SourceDocumentCommandHandler {
    processor: Arc<CommandProcessor<SourceDocument>>,
    id_generator: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
}

impl SourceDocumentCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<SourceDocument>>,
        id_generator: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            id_generator,
            clock,
        })
    }

    async fn dispatch(&self, command: SourceDocumentCommand) -> Result<(), AppError> {
        let stream_id = command.document_id();
        self.processor.handle(stream_id, command).await?;
        Ok(())
    }

    pub async fn create_document(
        &self,
        document_type: DocumentType,
        source_ref: SourceRef,
        content_hash: ContentHash,
        metadata: DocumentMetadata,
    ) -> Result<Uuid, AppError> {
        let document_id = self.id_generator.new_uuid();
        self.dispatch(SourceDocumentCommand::CreateDocument(CreateDocument {
            document_id,
            document_type,
            source_ref,
            initial_version: NewVersion {
                content_hash,
                metadata,
            },
            occurred_at: self.clock.now(),
        }))
        .await?;
        Ok(document_id)
    }

    pub async fn add_document_version(
        &self,
        document_id: Uuid,
        content_hash: ContentHash,
        metadata: DocumentMetadata,
    ) -> Result<(), AppError> {
        self.dispatch(SourceDocumentCommand::AddVersion(AddVersion {
            document_id,
            version: NewVersion {
                content_hash,
                metadata,
            },
            occurred_at: self.clock.now(),
        }))
        .await
    }

    pub async fn delete(&self, document_id: Uuid) -> Result<(), AppError> {
        self.dispatch(SourceDocumentCommand::DeleteDocument(DeleteDocument {
            document_id,
            occurred_at: self.clock.now(),
        }))
        .await
    }
}
