use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::Aggregate;

use crate::server::domain::shared::value_objects::ChunkingConfig;

use super::commands::ChunkSetCommand;
use super::events::{
    ChunkSetCreated, ChunkSetDeleted, ChunkSetEvent, ChunkSetPinned, ChunkSetUnpinned,
};
use super::exceptions::ChunkSetError;

const CHUNK_SET_NAMESPACE: Uuid = uuid::uuid!("d4f1a8b6-3c2e-4a1f-9e8d-7b5c4f2a6b91");

pub fn derive_chunk_set_id(
    document_id: Uuid,
    document_version: u32,
    config: &ChunkingConfig,
) -> Uuid {
    #[allow(clippy::expect_used)]
    let config_json = serde_json::to_string(config)
        .expect("ChunkingConfig serialisation must be infallible for chunk_set_id dedup");
    let name = format!("{document_id}|{document_version}|{config_json}");
    Uuid::new_v5(&CHUNK_SET_NAMESPACE, name.as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSet {
    pub chunk_set_id: Uuid,
    pub pinned: bool,
    pub deleted: bool,
}

impl ChunkSet {
    fn from_created(e: &ChunkSetCreated) -> Self {
        Self {
            chunk_set_id: e.chunk_set_id,
            pinned: false,
            deleted: false,
        }
    }
}

impl Aggregate for ChunkSet {
    type Event = ChunkSetEvent;
    type Command = ChunkSetCommand;
    type Error = ChunkSetError;

    fn aggregate_type() -> &'static str {
        "chunk_set"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ChunkSetEvent::ChunkSetCreated(_) => {}
            ChunkSetEvent::ChunkSetPinned(_) => self.pinned = true,
            ChunkSetEvent::ChunkSetUnpinned(_) => self.pinned = false,
            ChunkSetEvent::ChunkSetDeleted(_) => self.deleted = true,
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ChunkSetCommand::CreateChunkSet(cmd) => {
                if state.is_some() {
                    return Err(ChunkSetError::AlreadyExists);
                }
                if cmd.chunks.is_empty() {
                    return Err(ChunkSetError::ValidationError(
                        "chunk set must contain at least one chunk".into(),
                    ));
                }
                Ok(vec![ChunkSetEvent::ChunkSetCreated(ChunkSetCreated {
                    chunk_set_id: cmd.chunk_set_id,
                    document_id: cmd.document_id,
                    document_version: cmd.document_version,
                    chunking_config: cmd.chunking_config,
                    chunks: cmd.chunks,
                    occurred_at: cmd.occurred_at,
                })])
            }
            ChunkSetCommand::PinChunkSet(cmd) => {
                let cs = state.ok_or(ChunkSetError::NotFound)?;
                if cs.deleted {
                    return Err(ChunkSetError::AlreadyDeleted);
                }
                if cs.pinned {
                    return Ok(vec![]);
                }
                Ok(vec![ChunkSetEvent::ChunkSetPinned(ChunkSetPinned {
                    occurred_at: cmd.occurred_at,
                })])
            }
            ChunkSetCommand::UnpinChunkSet(cmd) => {
                let cs = state.ok_or(ChunkSetError::NotFound)?;
                if cs.deleted {
                    return Err(ChunkSetError::AlreadyDeleted);
                }
                if !cs.pinned {
                    return Ok(vec![]);
                }
                Ok(vec![ChunkSetEvent::ChunkSetUnpinned(ChunkSetUnpinned {
                    occurred_at: cmd.occurred_at,
                })])
            }
            ChunkSetCommand::DeleteChunkSet(cmd) => {
                let cs = state.ok_or(ChunkSetError::NotFound)?;
                if cs.deleted {
                    return Ok(vec![]);
                }
                if cs.pinned {
                    return Err(ChunkSetError::PinnedCannotDelete);
                }
                Ok(vec![ChunkSetEvent::ChunkSetDeleted(ChunkSetDeleted {
                    occurred_at: cmd.occurred_at,
                })])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, ChunkSetEvent::ChunkSetCreated(created)) => {
                    state = Some(Self::from_created(created));
                }
                (Some(_), ChunkSetEvent::ChunkSetCreated(_)) | (None, _) => return None,
                (Some(cs), event) => cs.apply(event),
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::chunk_set::chunk::Chunk;
    use crate::server::domain::shared::value_objects::SectionChunkingConfig;
    use crate::server::domain::shared::Timestamp;

    fn ts() -> Timestamp {
        "2026-01-01T00:00:00Z".into()
    }

    fn config() -> ChunkingConfig {
        ChunkingConfig::Section(SectionChunkingConfig {
            max_section_tokens: 512,
        })
    }

    fn chunk(seq: u32) -> Chunk {
        Chunk {
            chunk_id: Uuid::new_v4(),
            sequence: seq,
            heading: format!("H{seq}"),
            text: format!("text {seq}"),
            char_start: seq * 100,
            char_end: (seq + 1) * 100,
        }
    }

    fn create_cmd(id: Uuid) -> ChunkSetCommand {
        ChunkSetCommand::CreateChunkSet(CreateChunkSet {
            chunk_set_id: id,
            document_id: Uuid::new_v4(),
            document_version: 1,
            chunking_config: config(),
            chunks: vec![chunk(0), chunk(1)],
            occurred_at: ts(),
        })
    }

    use super::super::commands::{CreateChunkSet, DeleteChunkSet, PinChunkSet, UnpinChunkSet};

    #[test]
    fn create_sets_initial_state() {
        let id = Uuid::new_v4();
        let events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        assert_eq!(state.chunk_set_id, id);
        assert!(!state.pinned);
        assert!(!state.deleted);
    }

    #[test]
    fn create_twice_fails() {
        let id = Uuid::new_v4();
        let events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        let err = ChunkSet::handle_command(Some(&state), create_cmd(id)).unwrap_err();
        assert!(matches!(err, ChunkSetError::AlreadyExists));
    }

    #[test]
    fn create_with_empty_chunks_rejected() {
        let mut cmd = match create_cmd(Uuid::new_v4()) {
            ChunkSetCommand::CreateChunkSet(c) => c,
            _ => unreachable!(),
        };
        cmd.chunks.clear();
        let err = ChunkSet::handle_command(None, ChunkSetCommand::CreateChunkSet(cmd)).unwrap_err();
        assert!(matches!(err, ChunkSetError::ValidationError(_)));
    }

    #[test]
    fn pin_then_unpin_round_trip() {
        let id = Uuid::new_v4();
        let mut events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        let pin = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::PinChunkSet(PinChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap();
        events.extend(pin);
        let state = ChunkSet::from_events(&events).unwrap();
        assert!(state.pinned);

        let unpin = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::UnpinChunkSet(UnpinChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap();
        events.extend(unpin);
        let state = ChunkSet::from_events(&events).unwrap();
        assert!(!state.pinned);
    }

    #[test]
    fn pin_when_already_pinned_is_noop() {
        let id = Uuid::new_v4();
        let mut events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        let pin = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::PinChunkSet(PinChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap();
        events.extend(pin);
        let state = ChunkSet::from_events(&events).unwrap();
        let again = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::PinChunkSet(PinChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn delete_when_pinned_fails() {
        let id = Uuid::new_v4();
        let mut events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        events.extend(
            ChunkSet::handle_command(
                Some(&state),
                ChunkSetCommand::PinChunkSet(PinChunkSet {
                    chunk_set_id: id,
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = ChunkSet::from_events(&events).unwrap();
        let err = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::DeleteChunkSet(DeleteChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ChunkSetError::PinnedCannotDelete));
    }

    #[test]
    fn delete_when_unpinned_succeeds() {
        let id = Uuid::new_v4();
        let mut events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        let deleted = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::DeleteChunkSet(DeleteChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap();
        events.extend(deleted);
        let state = ChunkSet::from_events(&events).unwrap();
        assert!(state.deleted);
    }

    #[test]
    fn pin_after_delete_fails() {
        let id = Uuid::new_v4();
        let mut events = ChunkSet::handle_command(None, create_cmd(id)).unwrap();
        let state = ChunkSet::from_events(&events).unwrap();
        events.extend(
            ChunkSet::handle_command(
                Some(&state),
                ChunkSetCommand::DeleteChunkSet(DeleteChunkSet {
                    chunk_set_id: id,
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = ChunkSet::from_events(&events).unwrap();
        let err = ChunkSet::handle_command(
            Some(&state),
            ChunkSetCommand::PinChunkSet(PinChunkSet {
                chunk_set_id: id,
                occurred_at: ts(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ChunkSetError::AlreadyDeleted));
    }

    #[test]
    fn deterministic_id_is_stable() {
        let doc = Uuid::new_v4();
        let a = derive_chunk_set_id(doc, 3, &config());
        let b = derive_chunk_set_id(doc, 3, &config());
        assert_eq!(a, b);
    }

    #[test]
    fn deterministic_id_changes_with_version() {
        let doc = Uuid::new_v4();
        let a = derive_chunk_set_id(doc, 3, &config());
        let b = derive_chunk_set_id(doc, 4, &config());
        assert_ne!(a, b);
    }
}
