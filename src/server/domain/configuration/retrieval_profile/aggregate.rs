use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::{
    AddRetrievalProfile, RetrievalProfileCatalogCommand, UpdateRetrievalProfile,
};
use super::entity::RetrievalProfile;
use super::events::{
    RetrievalProfileAdded, RetrievalProfileCatalogCreated, RetrievalProfileCatalogEvent,
    RetrievalProfileRemoved, RetrievalProfileUpdated,
};

const RETRIEVAL_PROFILE_CATALOG_ID: Uuid = uuid::uuid!("4f1a7d63-7e9c-4a2b-b1d8-3a0f99e6d502");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalProfileCatalog {
    state: CatalogState<RetrievalProfile>,
}

impl RetrievalProfileCatalog {
    pub fn singleton_id() -> Uuid {
        RETRIEVAL_PROFILE_CATALOG_ID
    }

    pub fn entries(&self) -> &[RetrievalProfile] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

fn entry_from_add(id: Uuid, cmd: AddRetrievalProfile) -> RetrievalProfile {
    RetrievalProfile {
        retrieval_profile_id: id,
        name: cmd.name,
        index_profile_id: cmd.index_profile.index_profile_id,
        generation_model_id: cmd.generation_model.generation_model_id,
        reranker_model_id: cmd.reranker_model_id,
        default_top_k: cmd.default_top_k,
        default_min_score_milli: cmd.default_min_score_milli,
    }
}

fn entry_from_update(id: Uuid, cmd: UpdateRetrievalProfile) -> RetrievalProfile {
    RetrievalProfile {
        retrieval_profile_id: id,
        name: cmd.name,
        index_profile_id: cmd.index_profile.index_profile_id,
        generation_model_id: cmd.generation_model.generation_model_id,
        reranker_model_id: cmd.reranker_model_id,
        default_top_k: cmd.default_top_k,
        default_min_score_milli: cmd.default_min_score_milli,
    }
}

impl Aggregate for RetrievalProfileCatalog {
    type Event = RetrievalProfileCatalogEvent;
    type Command = RetrievalProfileCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "retrieval_profile_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::RetrievalProfileCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::RetrievalProfileAdded(e) => {
                self.state.push(RetrievalProfile {
                    retrieval_profile_id: e.retrieval_profile_id,
                    name: e.name.clone(),
                    index_profile_id: e.index_profile_id,
                    generation_model_id: e.generation_model_id,
                    reranker_model_id: e.reranker_model_id,
                    default_top_k: e.default_top_k,
                    default_min_score_milli: e.default_min_score_milli,
                });
            }
            Self::Event::RetrievalProfileUpdated(e) => {
                self.state.update(RetrievalProfile {
                    retrieval_profile_id: e.retrieval_profile_id,
                    name: e.name.clone(),
                    index_profile_id: e.index_profile_id,
                    generation_model_id: e.generation_model_id,
                    reranker_model_id: e.reranker_model_id,
                    default_top_k: e.default_top_k,
                    default_min_score_milli: e.default_min_score_milli,
                });
            }
            Self::Event::RetrievalProfileRemoved(e) => {
                self.state.remove(e.retrieval_profile_id);
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
            bootstrap.push(Self::Event::RetrievalProfileCatalogCreated(
                RetrievalProfileCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            RetrievalProfileCatalogCommand::AddRetrievalProfile(cmd) => {
                let candidate = entry_from_add(Uuid::new_v4(), cmd);
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::RetrievalProfileAdded(RetrievalProfileAdded {
                    retrieval_profile_id: candidate.retrieval_profile_id,
                    name: candidate.name,
                    index_profile_id: candidate.index_profile_id,
                    generation_model_id: candidate.generation_model_id,
                    reranker_model_id: candidate.reranker_model_id,
                    default_top_k: candidate.default_top_k,
                    default_min_score_milli: candidate.default_min_score_milli,
                })]
            }
            RetrievalProfileCatalogCommand::UpdateRetrievalProfile(cmd) => {
                let existing = owned_state.state.find(cmd.retrieval_profile_id)?;
                let candidate = entry_from_update(existing.retrieval_profile_id, cmd);
                candidate.validate()?;
                owned_state.state.ensure_unique(
                    &candidate.natural_key(),
                    Some(existing.retrieval_profile_id),
                )?;
                vec![Self::Event::RetrievalProfileUpdated(
                    RetrievalProfileUpdated {
                        retrieval_profile_id: candidate.retrieval_profile_id,
                        name: candidate.name,
                        index_profile_id: candidate.index_profile_id,
                        generation_model_id: candidate.generation_model_id,
                        reranker_model_id: candidate.reranker_model_id,
                        default_top_k: candidate.default_top_k,
                        default_min_score_milli: candidate.default_min_score_milli,
                    },
                )]
            }
            RetrievalProfileCatalogCommand::RemoveRetrievalProfile(cmd) => {
                let existing = owned_state.state.find(cmd.retrieval_profile_id)?;
                vec![Self::Event::RetrievalProfileRemoved(
                    RetrievalProfileRemoved {
                        retrieval_profile_id: existing.retrieval_profile_id,
                    },
                )]
            }
        };
        bootstrap.append(&mut events);
        Ok(bootstrap)
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, Self::Event::RetrievalProfileCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::RetrievalProfileCatalogCreated(_)) | (None, _) => {
                    return None
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<RetrievalProfileCatalog, ()> for RetrievalProfileCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::{GenerationModelRef, IndexProfileRef, RemoveRetrievalProfile};
    use super::*;

    fn add(name: &str, top_k: u32, min_score: u32) -> RetrievalProfileCatalogCommand {
        RetrievalProfileCatalogCommand::AddRetrievalProfile(AddRetrievalProfile {
            name: name.into(),
            index_profile: IndexProfileRef {
                index_profile_id: Uuid::new_v4(),
            },
            generation_model: GenerationModelRef {
                generation_model_id: Uuid::new_v4(),
            },
            reranker_model_id: None,
            default_top_k: top_k,
            default_min_score_milli: min_score,
        })
    }

    #[test]
    fn first_add_bootstraps_catalog() {
        let events = RetrievalProfileCatalog::handle_command(None, add("default", 5, 300)).unwrap();
        assert!(matches!(
            events[0],
            RetrievalProfileCatalogEvent::RetrievalProfileCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            RetrievalProfileCatalogEvent::RetrievalProfileAdded(_)
        ));
    }

    #[test]
    fn duplicate_name_rejected() {
        let events = RetrievalProfileCatalog::handle_command(None, add("default", 5, 300)).unwrap();
        let state = RetrievalProfileCatalog::from_events(&events).unwrap();
        let err = RetrievalProfileCatalog::handle_command(Some(&state), add("default", 5, 300))
            .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn empty_name_rejected() {
        let err = RetrievalProfileCatalog::handle_command(None, add("  ", 5, 300)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn zero_top_k_rejected() {
        let err = RetrievalProfileCatalog::handle_command(None, add("zero", 0, 300)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn min_score_above_1000_rejected() {
        let err = RetrievalProfileCatalog::handle_command(None, add("hi", 5, 1500)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn remove_drops_entry() {
        let events = RetrievalProfileCatalog::handle_command(None, add("default", 5, 300)).unwrap();
        let state_after_add = RetrievalProfileCatalog::from_events(&events).unwrap();
        let id = state_after_add.entries()[0].retrieval_profile_id;
        let remove_events = RetrievalProfileCatalog::handle_command(
            Some(&state_after_add),
            RetrievalProfileCatalogCommand::RemoveRetrievalProfile(RemoveRetrievalProfile {
                retrieval_profile_id: id,
            }),
        )
        .unwrap();
        assert!(matches!(
            remove_events[0],
            RetrievalProfileCatalogEvent::RetrievalProfileRemoved(_)
        ));
    }
}
