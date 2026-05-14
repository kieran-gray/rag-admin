use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::{
    document_type::DocumentType,
    source_ref::SourceRef,
    version::{ContentHash, DocumentMetadata},
};

pub struct NewVersion {
    pub content_hash: ContentHash,
    pub metadata: DocumentMetadata,
}

pub struct CreateDocument {
    pub document_id: Uuid,
    pub document_type: DocumentType,
    pub source_ref: SourceRef,
    pub initial_version: NewVersion,
    pub occurred_at: Timestamp,
}

pub struct AddVersion {
    pub document_id: Uuid,
    pub version: NewVersion,
    pub occurred_at: Timestamp,
}

pub struct DeleteDocument {
    pub document_id: Uuid,
    pub occurred_at: Timestamp,
}

pub enum SourceDocumentCommand {
    CreateDocument(CreateDocument),
    AddVersion(AddVersion),
    DeleteDocument(DeleteDocument),
}

impl SourceDocumentCommand {
    pub fn document_id(&self) -> Uuid {
        match self {
            SourceDocumentCommand::CreateDocument(cmd) => cmd.document_id,
            SourceDocumentCommand::AddVersion(cmd) => cmd.document_id,
            SourceDocumentCommand::DeleteDocument(cmd) => cmd.document_id,
        }
    }
}
