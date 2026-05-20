use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::VectorIndexCatalogCommand;
use super::entity::VectorIndex;
use super::events::{
    VectorIndexAdded, VectorIndexCatalogCreated, VectorIndexCatalogEvent, VectorIndexRemoved,
    VectorIndexUpdated,
};

const VECTOR_INDEX_CATALOG_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a003");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexCatalog {
    state: CatalogState<VectorIndex>,
}

impl VectorIndexCatalog {
    pub fn singleton_id() -> Uuid {
        VECTOR_INDEX_CATALOG_ID
    }

    pub fn indexes(&self) -> &[VectorIndex] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

impl Aggregate for VectorIndexCatalog {
    type Event = VectorIndexCatalogEvent;
    type Command = VectorIndexCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "vector_index_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::VectorIndexCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::VectorIndexAdded(e) => {
                self.state.push(VectorIndex {
                    index_id: e.index_id,
                    kind: e.kind,
                    name: e.name.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::VectorIndexUpdated(e) => {
                self.state.update(VectorIndex {
                    index_id: e.index_id,
                    kind: e.kind,
                    name: e.name.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::VectorIndexRemoved(e) => {
                self.state.remove(e.index_id);
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        let mut bootstrap = Vec::new();
        let owned_state = if let Some(s) = state {
            s.clone()
        } else {
            bootstrap.push(Self::Event::VectorIndexCatalogCreated(
                VectorIndexCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            VectorIndexCatalogCommand::AddVectorIndex(cmd) => {
                let candidate = VectorIndex {
                    index_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    name: cmd.name,
                    dimensions: cmd.dimensions,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::VectorIndexAdded(VectorIndexAdded {
                    index_id: candidate.index_id,
                    kind: candidate.kind,
                    name: candidate.name,
                    dimensions: candidate.dimensions,
                })]
            }
            VectorIndexCatalogCommand::UpdateVectorIndex(cmd) => {
                let existing = owned_state.state.find(cmd.index_id)?;
                let candidate = VectorIndex {
                    index_id: existing.index_id,
                    kind: cmd.kind,
                    name: cmd.name,
                    dimensions: cmd.dimensions,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), Some(existing.index_id))?;
                vec![Self::Event::VectorIndexUpdated(VectorIndexUpdated {
                    index_id: candidate.index_id,
                    kind: candidate.kind,
                    name: candidate.name,
                    dimensions: candidate.dimensions,
                })]
            }
            VectorIndexCatalogCommand::RemoveVectorIndex(cmd) => {
                let existing = owned_state.state.find(cmd.index_id)?;
                vec![Self::Event::VectorIndexRemoved(VectorIndexRemoved {
                    index_id: existing.index_id,
                })]
            }
        };
        bootstrap.append(&mut events);
        Ok(bootstrap)
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, Self::Event::VectorIndexCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::VectorIndexCatalogCreated(_)) | (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<VectorIndexCatalog, ()> for VectorIndexCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::{AddVectorIndex, RemoveVectorIndex, UpdateVectorIndex};
    use super::*;
    use crate::shared::reference_data::VectorStoreKind;

    fn add_cmd(name: &str, dim: u32) -> VectorIndexCatalogCommand {
        VectorIndexCatalogCommand::AddVectorIndex(AddVectorIndex {
            kind: VectorStoreKind::Postgres,
            name: name.into(),
            dimensions: dim,
        })
    }

    fn seeded() -> (VectorIndexCatalog, Vec<VectorIndexCatalogEvent>) {
        let events = VectorIndexCatalog::handle_command(None, add_cmd("docs", 768)).expect("seed");
        let state = VectorIndexCatalog::from_events(&events).expect("seed state");
        (state, events)
    }

    #[test]
    fn first_add_bootstraps_catalog_and_appends_entry() {
        let events = VectorIndexCatalog::handle_command(None, add_cmd("docs", 768)).unwrap();
        assert!(matches!(
            events[0],
            VectorIndexCatalogEvent::VectorIndexCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            VectorIndexCatalogEvent::VectorIndexAdded(_)
        ));
        let state = VectorIndexCatalog::from_events(&events).unwrap();
        assert_eq!(state.indexes().len(), 1);
        assert_eq!(state.indexes()[0].name, "docs");
    }

    #[test]
    fn second_add_does_not_re_bootstrap() {
        let (state, _) = seeded();
        let events =
            VectorIndexCatalog::handle_command(Some(&state), add_cmd("other", 1024)).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            VectorIndexCatalogEvent::VectorIndexAdded(_)
        ));
    }

    #[test]
    fn add_with_empty_name_rejected_with_validation_error() {
        let err = VectorIndexCatalog::handle_command(None, add_cmd("  ", 768)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn add_with_zero_dimensions_rejected() {
        let err = VectorIndexCatalog::handle_command(None, add_cmd("docs", 0)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn duplicate_name_rejected_regardless_of_dimensions() {
        let (state, _) = seeded();
        let err =
            VectorIndexCatalog::handle_command(Some(&state), add_cmd("docs", 1024)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_rewrites_entry_in_place() {
        let (state, mut events) = seeded();
        let existing_id = state.indexes()[0].index_id;
        let update_events = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::UpdateVectorIndex(UpdateVectorIndex {
                index_id: existing_id,
                kind: VectorStoreKind::CloudflareVectorize,
                name: "renamed".into(),
                dimensions: 1024,
            }),
        )
        .unwrap();
        events.extend(update_events);
        let updated = VectorIndexCatalog::from_events(&events).unwrap();
        let entry = &updated.indexes()[0];
        assert_eq!(entry.index_id, existing_id);
        assert_eq!(entry.name, "renamed");
        assert_eq!(entry.dimensions, 1024);
        assert_eq!(entry.kind, VectorStoreKind::CloudflareVectorize);
    }

    #[test]
    fn update_keeps_own_name_unique_against_self() {
        let (state, mut events) = seeded();
        let id = state.indexes()[0].index_id;
        let update_events = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::UpdateVectorIndex(UpdateVectorIndex {
                index_id: id,
                kind: VectorStoreKind::Postgres,
                name: "docs".into(),
                dimensions: 1024,
            }),
        )
        .unwrap();
        events.extend(update_events);
        let updated = VectorIndexCatalog::from_events(&events).unwrap();
        assert_eq!(updated.indexes()[0].dimensions, 1024);
    }

    #[test]
    fn update_collides_when_taking_another_entrys_name() {
        let (state, mut events) = seeded();
        let other =
            VectorIndexCatalog::handle_command(Some(&state), add_cmd("other", 768)).unwrap();
        events.extend(other);
        let state = VectorIndexCatalog::from_events(&events).unwrap();
        let other_id = state
            .indexes()
            .iter()
            .find(|e| e.name == "other")
            .unwrap()
            .index_id;
        let err = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::UpdateVectorIndex(UpdateVectorIndex {
                index_id: other_id,
                kind: VectorStoreKind::Postgres,
                name: "docs".into(),
                dimensions: 768,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::UpdateVectorIndex(UpdateVectorIndex {
                index_id: Uuid::new_v4(),
                kind: VectorStoreKind::Postgres,
                name: "x".into(),
                dimensions: 768,
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn remove_drops_entry() {
        let (state, mut events) = seeded();
        let id = state.indexes()[0].index_id;
        let rm = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::RemoveVectorIndex(RemoveVectorIndex { index_id: id }),
        )
        .unwrap();
        events.extend(rm);
        let after = VectorIndexCatalog::from_events(&events).unwrap();
        assert!(after.indexes().is_empty());
    }

    #[test]
    fn remove_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = VectorIndexCatalog::handle_command(
            Some(&state),
            VectorIndexCatalogCommand::RemoveVectorIndex(RemoveVectorIndex {
                index_id: Uuid::new_v4(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn replay_requires_catalog_created_first() {
        let events = vec![VectorIndexCatalogEvent::VectorIndexAdded(
            VectorIndexAdded {
                index_id: Uuid::new_v4(),
                kind: VectorStoreKind::Postgres,
                name: "x".into(),
                dimensions: 768,
            },
        )];
        assert!(VectorIndexCatalog::from_events(&events).is_none());
    }

    #[test]
    fn replay_rejects_duplicate_catalog_created() {
        let events = vec![
            VectorIndexCatalogEvent::VectorIndexCatalogCreated(VectorIndexCatalogCreated {
                catalog_id: VectorIndexCatalog::singleton_id(),
            }),
            VectorIndexCatalogEvent::VectorIndexCatalogCreated(VectorIndexCatalogCreated {
                catalog_id: VectorIndexCatalog::singleton_id(),
            }),
        ];
        assert!(VectorIndexCatalog::from_events(&events).is_none());
    }

    #[test]
    fn aggregate_type_is_stable_identifier() {
        assert_eq!(VectorIndexCatalog::aggregate_type(), "vector_index_catalog");
    }

    #[test]
    fn singleton_id_is_constant() {
        assert_eq!(
            VectorIndexCatalog::singleton_id(),
            VectorIndexCatalog::singleton_id(),
        );
    }
}
