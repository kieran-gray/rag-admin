use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event_sourcing::policy::HasPolicies;
use crate::event_sourcing::Aggregate;
use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};

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
