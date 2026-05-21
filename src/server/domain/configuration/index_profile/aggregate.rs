use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::{
    AddIndexProfile, EmbeddingModelRef, IndexProfileCatalogCommand, UpdateIndexProfile,
    VectorIndexRef,
};
use super::entity::IndexProfile;
use super::events::{
    IndexProfileAdded, IndexProfileCatalogCreated, IndexProfileCatalogEvent, IndexProfileRemoved,
    IndexProfileUpdated,
};

const INDEX_PROFILE_CATALOG_ID: Uuid = uuid::uuid!("3c2bb1d5-4c4e-4c8e-8c3a-2f6f76d8c401");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProfileCatalog {
    state: CatalogState<IndexProfile>,
}

impl IndexProfileCatalog {
    pub fn singleton_id() -> Uuid {
        INDEX_PROFILE_CATALOG_ID
    }

    pub fn entries(&self) -> &[IndexProfile] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

fn ensure_dimensions_match(
    embedding: &EmbeddingModelRef,
    vector_index: &VectorIndexRef,
) -> Result<u32, CatalogError> {
    if embedding.dimensions != vector_index.dimensions {
        return Err(CatalogError::ValidationError(format!(
            "embedding model dimensions ({}) do not match vector index dimensions ({})",
            embedding.dimensions, vector_index.dimensions
        )));
    }
    Ok(embedding.dimensions)
}

fn entry_from_add(id: Uuid, cmd: AddIndexProfile) -> Result<IndexProfile, CatalogError> {
    let dimensions = ensure_dimensions_match(&cmd.embedding_model, &cmd.vector_index)?;
    Ok(IndexProfile {
        index_profile_id: id,
        name: cmd.name,
        embedding_model_id: cmd.embedding_model.embedding_model_id,
        vector_index_id: cmd.vector_index.vector_index_id,
        dimensions,
    })
}

fn entry_from_update(id: Uuid, cmd: UpdateIndexProfile) -> Result<IndexProfile, CatalogError> {
    let dimensions = ensure_dimensions_match(&cmd.embedding_model, &cmd.vector_index)?;
    Ok(IndexProfile {
        index_profile_id: id,
        name: cmd.name,
        embedding_model_id: cmd.embedding_model.embedding_model_id,
        vector_index_id: cmd.vector_index.vector_index_id,
        dimensions,
    })
}

impl Aggregate for IndexProfileCatalog {
    type Event = IndexProfileCatalogEvent;
    type Command = IndexProfileCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "index_profile_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::IndexProfileCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::IndexProfileAdded(e) => {
                self.state.push(IndexProfile {
                    index_profile_id: e.index_profile_id,
                    name: e.name.clone(),
                    embedding_model_id: e.embedding_model_id,
                    vector_index_id: e.vector_index_id,
                    dimensions: e.dimensions,
                });
            }
            Self::Event::IndexProfileUpdated(e) => {
                self.state.update(IndexProfile {
                    index_profile_id: e.index_profile_id,
                    name: e.name.clone(),
                    embedding_model_id: e.embedding_model_id,
                    vector_index_id: e.vector_index_id,
                    dimensions: e.dimensions,
                });
            }
            Self::Event::IndexProfileRemoved(e) => {
                self.state.remove(e.index_profile_id);
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
            bootstrap.push(Self::Event::IndexProfileCatalogCreated(
                IndexProfileCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            IndexProfileCatalogCommand::AddIndexProfile(cmd) => {
                let candidate = entry_from_add(Uuid::new_v4(), cmd)?;
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::IndexProfileAdded(IndexProfileAdded {
                    index_profile_id: candidate.index_profile_id,
                    name: candidate.name,
                    embedding_model_id: candidate.embedding_model_id,
                    vector_index_id: candidate.vector_index_id,
                    dimensions: candidate.dimensions,
                })]
            }
            IndexProfileCatalogCommand::UpdateIndexProfile(cmd) => {
                let existing = owned_state.state.find(cmd.index_profile_id)?;
                let candidate = entry_from_update(existing.index_profile_id, cmd)?;
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), Some(existing.index_profile_id))?;
                vec![Self::Event::IndexProfileUpdated(IndexProfileUpdated {
                    index_profile_id: candidate.index_profile_id,
                    name: candidate.name,
                    embedding_model_id: candidate.embedding_model_id,
                    vector_index_id: candidate.vector_index_id,
                    dimensions: candidate.dimensions,
                })]
            }
            IndexProfileCatalogCommand::RemoveIndexProfile(cmd) => {
                let existing = owned_state.state.find(cmd.index_profile_id)?;
                vec![Self::Event::IndexProfileRemoved(IndexProfileRemoved {
                    index_profile_id: existing.index_profile_id,
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
                (None, Self::Event::IndexProfileCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::IndexProfileCatalogCreated(_)) | (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<IndexProfileCatalog, ()> for IndexProfileCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::RemoveIndexProfile;
    use super::*;

    fn add(name: &str, em_dim: u32, vi_dim: u32) -> IndexProfileCatalogCommand {
        IndexProfileCatalogCommand::AddIndexProfile(AddIndexProfile {
            name: name.into(),
            embedding_model: EmbeddingModelRef {
                embedding_model_id: Uuid::new_v4(),
                dimensions: em_dim,
            },
            vector_index: VectorIndexRef {
                vector_index_id: Uuid::new_v4(),
                dimensions: vi_dim,
            },
        })
    }

    #[test]
    fn first_add_bootstraps_catalog() {
        let events = IndexProfileCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        assert!(matches!(
            events[0],
            IndexProfileCatalogEvent::IndexProfileCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            IndexProfileCatalogEvent::IndexProfileAdded(_)
        ));
    }

    #[test]
    fn dimension_mismatch_rejected() {
        let err = IndexProfileCatalog::handle_command(None, add("bad", 1024, 768)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn duplicate_name_rejected() {
        let events = IndexProfileCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        let state = IndexProfileCatalog::from_events(&events).unwrap();
        let err = IndexProfileCatalog::handle_command(Some(&state), add("default", 1024, 1024))
            .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn empty_name_rejected() {
        let err = IndexProfileCatalog::handle_command(None, add("  ", 1024, 1024)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn remove_drops_entry() {
        let events = IndexProfileCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        let state_after_add = IndexProfileCatalog::from_events(&events).unwrap();
        let id = state_after_add.entries()[0].index_profile_id;
        let remove_events = IndexProfileCatalog::handle_command(
            Some(&state_after_add),
            IndexProfileCatalogCommand::RemoveIndexProfile(RemoveIndexProfile {
                index_profile_id: id,
            }),
        )
        .unwrap();
        assert!(matches!(
            remove_events[0],
            IndexProfileCatalogEvent::IndexProfileRemoved(_)
        ));
    }
}
