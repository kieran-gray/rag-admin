use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::{
    commands::SourceDocumentCommand,
    events::{DocumentCreated, DocumentDeleted, SourceDocumentEvent, VersionAdded},
    exceptions::SourceDocumentError,
    version::ContentHash,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocument {
    pub document_id: Uuid,
    pub latest_content_hash: Option<ContentHash>,
    pub latest_version_number: u32,
    pub deleted: bool,
}

impl SourceDocument {
    fn from_created(cmd: &DocumentCreated) -> Self {
        Self {
            document_id: cmd.document_id,
            latest_content_hash: None,
            latest_version_number: 0,
            deleted: false,
        }
    }
}

impl Aggregate for SourceDocument {
    type Event = SourceDocumentEvent;
    type Command = SourceDocumentCommand;
    type Error = SourceDocumentError;

    fn aggregate_type() -> &'static str {
        "source_document"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::DocumentCreated(_) => {}
            Self::Event::VersionAdded(e) => {
                self.latest_version_number = e.version_number;
                self.latest_content_hash = Some(e.content_hash.clone());
            }
            Self::Event::DocumentDeleted(_) => {
                self.deleted = true;
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::CreateDocument(cmd) => {
                if state.is_some() {
                    return Err(SourceDocumentError::AlreadyExists);
                }
                let created = DocumentCreated {
                    document_id: cmd.document_id,
                    document_type: cmd.document_type.clone(),
                    source_ref: cmd.source_ref.clone(),
                    occurred_at: cmd.occurred_at.clone(),
                };
                let version_number = 1;
                let version_added = VersionAdded {
                    version_number,
                    content_hash: cmd.initial_version.content_hash,
                    metadata: cmd.initial_version.metadata,
                    occurred_at: cmd.occurred_at,
                };
                Ok(vec![
                    Self::Event::DocumentCreated(created),
                    Self::Event::VersionAdded(version_added),
                ])
            }

            Self::Command::AddVersion(cmd) => {
                let doc = state.ok_or(SourceDocumentError::NotFound)?;
                if doc.deleted {
                    return Err(SourceDocumentError::AlreadyDeleted);
                }
                if doc.latest_content_hash.as_ref() == Some(&cmd.version.content_hash) {
                    return Ok(vec![]);
                }
                let version_number = doc.latest_version_number.saturating_add(1).max(1);
                Ok(vec![Self::Event::VersionAdded(VersionAdded {
                    version_number,
                    content_hash: cmd.version.content_hash,
                    metadata: cmd.version.metadata,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::DeleteDocument(cmd) => {
                let doc = state.ok_or(SourceDocumentError::NotFound)?;
                if doc.deleted {
                    return Err(SourceDocumentError::AlreadyDeleted);
                }
                Ok(vec![Self::Event::DocumentDeleted(DocumentDeleted {
                    occurred_at: cmd.occurred_at,
                })])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;

        for event in events {
            match (&mut state, event) {
                (None, Self::Event::DocumentCreated(created)) => {
                    state = Some(Self::from_created(created));
                }
                (Some(_), Self::Event::DocumentCreated(_)) | (None, _) => return None,
                (Some(doc), event) => doc.apply(event),
            }
        }

        state
    }
}

impl HasPolicies<SourceDocument, ()> for SourceDocumentEvent {}

impl SourceDocument {
    #[cfg(test)]
    pub fn test_create(document_id: Uuid, url: &str) -> SourceDocumentCommand {
        use super::{
            commands::{CreateDocument, NewVersion},
            document_type::DocumentType,
            source_ref::SourceRef,
            version::{DocumentMetadata, WebPageMetadata},
        };
        SourceDocumentCommand::CreateDocument(CreateDocument {
            document_id,
            document_type: DocumentType::WebPage,
            source_ref: SourceRef::Url {
                url: url.to_string(),
            },
            initial_version: NewVersion {
                content_hash: ContentHash::new("abc123".to_string()),
                metadata: DocumentMetadata::WebPage(WebPageMetadata {
                    title: "Test Page".to_string(),
                    source_url: url.to_string(),
                    slug: "test-page".to_string(),
                    fetched_at: "2024-01-01T00:00:00Z".into(),
                }),
            },
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::{
        shared::Timestamp,
        source_document::{
            commands::{AddVersion, DeleteDocument, NewVersion},
            document_type::DocumentType,
            events::{DocumentCreated, VersionAdded},
            source_ref::SourceRef,
            version::{DocumentMetadata, WebPageMetadata},
        },
    };

    fn make_created_events(document_id: Uuid, url: &str) -> Vec<SourceDocumentEvent> {
        vec![
            SourceDocumentEvent::DocumentCreated(DocumentCreated {
                document_id,
                document_type: DocumentType::WebPage,
                source_ref: SourceRef::Url {
                    url: url.to_string(),
                },
                occurred_at: now(),
            }),
            SourceDocumentEvent::VersionAdded(VersionAdded {
                version_number: 1,
                content_hash: ContentHash::new("abc123".to_string()),
                metadata: make_metadata(url),
                occurred_at: now(),
            }),
        ]
    }

    fn now() -> Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn make_hash(s: &str) -> ContentHash {
        ContentHash::new(s.to_string())
    }

    fn make_metadata(url: &str) -> DocumentMetadata {
        DocumentMetadata::WebPage(WebPageMetadata {
            title: "My Page".to_string(),
            source_url: url.to_string(),
            slug: "my-page".to_string(),
            fetched_at: now(),
        })
    }

    #[test]
    fn create_document_emits_created_and_version_added() {
        let id = Uuid::new_v4();
        let events = SourceDocument::handle_command(
            None,
            SourceDocument::test_create(id, "https://example.com/a"),
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], SourceDocumentEvent::DocumentCreated(_)));
        assert!(matches!(events[1], SourceDocumentEvent::VersionAdded(_)));

        let doc = SourceDocument::from_events(&events).unwrap();
        assert_eq!(doc.document_id, id);
        assert_eq!(doc.latest_version_number, 1);
    }

    #[test]
    fn creating_already_existing_document_fails() {
        let id = Uuid::new_v4();
        let events = make_created_events(id, "https://example.com/a");
        let doc = SourceDocument::from_events(&events).unwrap();

        let err = SourceDocument::handle_command(
            Some(&doc),
            SourceDocument::test_create(id, "https://example.com/a"),
        )
        .unwrap_err();

        assert!(matches!(err, SourceDocumentError::AlreadyExists));
    }

    #[test]
    fn add_version_increments_version_number() {
        let id = Uuid::new_v4();
        let events = make_created_events(id, "https://example.com/a");
        let doc = SourceDocument::from_events(&events).unwrap();

        let new_events = SourceDocument::handle_command(
            Some(&doc),
            SourceDocumentCommand::AddVersion(AddVersion {
                document_id: id,
                version: NewVersion {
                    content_hash: make_hash("def456"),
                    metadata: make_metadata("https://example.com/a"),
                },
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert_eq!(new_events.len(), 1);
        if let SourceDocumentEvent::VersionAdded(v) = &new_events[0] {
            assert_eq!(v.version_number, 2);
            assert_eq!(v.content_hash, make_hash("def456"));
        } else {
            panic!("expected VersionAdded");
        }
    }

    #[test]
    fn add_version_with_identical_hash_is_idempotent() {
        let id = Uuid::new_v4();
        let events = make_created_events(id, "https://example.com/a");
        let doc = SourceDocument::from_events(&events).unwrap();

        let new_events = SourceDocument::handle_command(
            Some(&doc),
            SourceDocumentCommand::AddVersion(AddVersion {
                document_id: id,
                version: NewVersion {
                    content_hash: make_hash("abc123"),
                    metadata: make_metadata("https://example.com/a"),
                },
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert!(new_events.is_empty());
    }

    #[test]
    fn add_version_on_missing_document_fails() {
        let id = Uuid::new_v4();
        let err = SourceDocument::handle_command(
            None,
            SourceDocumentCommand::AddVersion(AddVersion {
                document_id: id,
                version: NewVersion {
                    content_hash: make_hash("abc123"),
                    metadata: make_metadata("https://example.com/a"),
                },
                occurred_at: now(),
            }),
        )
        .unwrap_err();

        assert!(matches!(err, SourceDocumentError::NotFound));
    }

    #[test]
    fn delete_emits_document_deleted() {
        let id = Uuid::new_v4();
        let events = make_created_events(id, "https://example.com/a");
        let doc = SourceDocument::from_events(&events).unwrap();

        let new_events = SourceDocument::handle_command(
            Some(&doc),
            SourceDocumentCommand::DeleteDocument(DeleteDocument {
                document_id: id,
                occurred_at: now(),
            }),
        )
        .unwrap();

        assert_eq!(new_events.len(), 1);
        assert!(matches!(
            new_events[0],
            SourceDocumentEvent::DocumentDeleted(_)
        ));
    }

    #[test]
    fn double_delete_fails() {
        let id = Uuid::new_v4();
        let mut events = make_created_events(id, "https://example.com/a");
        events.push(SourceDocumentEvent::DocumentDeleted(DocumentDeleted {
            occurred_at: now(),
        }));
        let doc = SourceDocument::from_events(&events).unwrap();

        let err = SourceDocument::handle_command(
            Some(&doc),
            SourceDocumentCommand::DeleteDocument(DeleteDocument {
                document_id: id,
                occurred_at: now(),
            }),
        )
        .unwrap_err();

        assert!(matches!(err, SourceDocumentError::AlreadyDeleted));
    }

    #[test]
    fn add_version_on_deleted_document_fails() {
        let id = Uuid::new_v4();
        let mut events = make_created_events(id, "https://example.com/a");
        events.push(SourceDocumentEvent::DocumentDeleted(DocumentDeleted {
            occurred_at: now(),
        }));
        let doc = SourceDocument::from_events(&events).unwrap();

        let err = SourceDocument::handle_command(
            Some(&doc),
            SourceDocumentCommand::AddVersion(AddVersion {
                document_id: id,
                version: NewVersion {
                    content_hash: make_hash("new123"),
                    metadata: make_metadata("https://example.com/a"),
                },
                occurred_at: now(),
            }),
        )
        .unwrap_err();

        assert!(matches!(err, SourceDocumentError::AlreadyDeleted));
    }

    #[test]
    fn replay_requires_document_created_as_first_event() {
        let result =
            SourceDocument::from_events(&[SourceDocumentEvent::VersionAdded(VersionAdded {
                version_number: 1,
                content_hash: make_hash("abc"),
                metadata: make_metadata("https://example.com/a"),
                occurred_at: now(),
            })]);

        assert!(result.is_none());
    }

    #[test]
    fn full_event_replay_is_consistent() {
        let id = Uuid::new_v4();
        let mut events = make_created_events(id, "https://example.com/a");
        events.push(SourceDocumentEvent::VersionAdded(VersionAdded {
            version_number: 2,
            content_hash: make_hash("v2"),
            metadata: make_metadata("https://example.com/a"),
            occurred_at: now(),
        }));

        let doc = SourceDocument::from_events(&events).unwrap();
        assert_eq!(doc.latest_version_number, 2);
        assert_eq!(doc.latest_content_hash.as_ref(), Some(&make_hash("v2")));
        assert!(!doc.deleted);
    }
}
