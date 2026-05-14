use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::event_sourcing::Aggregate;

use super::commands::VectorIndexCatalogCommand;
use super::entity::VectorIndex;
use super::events::{
    VectorIndexAdded, VectorIndexCatalogCreated, VectorIndexCatalogEvent, VectorIndexRemoved,
    VectorIndexUpdated,
};
use super::exceptions::VectorIndexCatalogError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexCatalog {
    pub catalog_id: Uuid,
    pub indexes: Vec<VectorIndex>,
}

impl VectorIndexCatalog {
    pub fn singleton_id() -> Uuid {
        Uuid::nil()
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            catalog_id,
            indexes: Vec::new(),
        }
    }

    fn find(&self, index_id: Uuid) -> Result<&VectorIndex, VectorIndexCatalogError> {
        self.indexes
            .iter()
            .find(|i| i.index_id == index_id)
            .ok_or(VectorIndexCatalogError::NotFound)
    }

    fn ensure_unique_name(
        &self,
        name: &str,
        excluding: Option<Uuid>,
    ) -> Result<(), VectorIndexCatalogError> {
        if self
            .indexes
            .iter()
            .any(|i| i.name == name && Some(i.index_id) != excluding)
        {
            return Err(VectorIndexCatalogError::ValidationError(format!(
                "Vector index with name {name} already exists"
            )));
        }
        Ok(())
    }
}

impl Aggregate for VectorIndexCatalog {
    type Event = VectorIndexCatalogEvent;
    type Command = VectorIndexCatalogCommand;
    type Error = VectorIndexCatalogError;

    fn aggregate_type() -> &'static str {
        "vector_index_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::VectorIndexCatalogCreated(e) => {
                *self = Self::empty(e.catalog_id);
            }
            Self::Event::VectorIndexAdded(e) => {
                self.indexes.push(VectorIndex {
                    index_id: e.index_id,
                    kind: e.kind,
                    name: e.name.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::VectorIndexUpdated(e) => {
                if let Some(i) = self.indexes.iter_mut().find(|i| i.index_id == e.index_id) {
                    i.kind = e.kind;
                    i.name = e.name.clone();
                    i.dimensions = e.dimensions;
                }
            }
            Self::Event::VectorIndexRemoved(e) => {
                self.indexes.retain(|i| i.index_id != e.index_id);
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        let mut bootstrap = Vec::new();
        let owned_state = match state {
            Some(s) => s.clone(),
            None => {
                bootstrap.push(Self::Event::VectorIndexCatalogCreated(
                    VectorIndexCatalogCreated {
                        catalog_id: Self::singleton_id(),
                    },
                ));
                Self::empty(Self::singleton_id())
            }
        };
        let state = &owned_state;

        let mut events = match command {
            VectorIndexCatalogCommand::AddVectorIndex(cmd) => {
                validate_non_empty("vector index name", &cmd.name)?;
                validate_positive("vector index dimensions", cmd.dimensions)?;
                state.ensure_unique_name(&cmd.name, None)?;
                vec![Self::Event::VectorIndexAdded(VectorIndexAdded {
                    index_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    name: cmd.name,
                    dimensions: cmd.dimensions,
                })]
            }
            VectorIndexCatalogCommand::UpdateVectorIndex(cmd) => {
                let existing = state.find(cmd.index_id)?;
                validate_non_empty("vector index name", &cmd.name)?;
                validate_positive("vector index dimensions", cmd.dimensions)?;
                state.ensure_unique_name(&cmd.name, Some(existing.index_id))?;
                vec![Self::Event::VectorIndexUpdated(VectorIndexUpdated {
                    index_id: existing.index_id,
                    kind: cmd.kind,
                    name: cmd.name,
                    dimensions: cmd.dimensions,
                })]
            }
            VectorIndexCatalogCommand::RemoveVectorIndex(cmd) => {
                let existing = state.find(cmd.index_id)?;
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
                (Some(_), Self::Event::VectorIndexCatalogCreated(_)) => return None,
                (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), VectorIndexCatalogError> {
    if value.trim().is_empty() {
        return Err(VectorIndexCatalogError::ValidationError(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(())
}

fn validate_positive(field: &str, value: u32) -> Result<(), VectorIndexCatalogError> {
    if value == 0 {
        return Err(VectorIndexCatalogError::ValidationError(format!(
            "{field} must be greater than zero"
        )));
    }
    Ok(())
}
