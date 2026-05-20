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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::source_document::version::PlainMetadata;

    fn ts() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn version() -> NewVersion {
        NewVersion {
            content_hash: ContentHash::new("h".into()),
            metadata: DocumentMetadata::PlainText(PlainMetadata { title: "t".into() }),
        }
    }

    #[test]
    fn document_id_returns_create_document_id() {
        let id = Uuid::new_v4();
        let cmd = SourceDocumentCommand::CreateDocument(CreateDocument {
            document_id: id,
            document_type: DocumentType::Markdown,
            source_ref: SourceRef::Upload {
                upload_id: Uuid::nil(),
            },
            initial_version: version(),
            occurred_at: ts(),
        });
        assert_eq!(cmd.document_id(), id);
    }

    #[test]
    fn document_id_returns_add_version_id() {
        let id = Uuid::new_v4();
        let cmd = SourceDocumentCommand::AddVersion(AddVersion {
            document_id: id,
            version: version(),
            occurred_at: ts(),
        });
        assert_eq!(cmd.document_id(), id);
    }

    #[test]
    fn document_id_returns_delete_document_id() {
        let id = Uuid::new_v4();
        let cmd = SourceDocumentCommand::DeleteDocument(DeleteDocument {
            document_id: id,
            occurred_at: ts(),
        });
        assert_eq!(cmd.document_id(), id);
    }
}
