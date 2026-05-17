use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event_sourcing::policy::HasPolicies;
use crate::event_sourcing::Aggregate;
use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};

use super::commands::GenerationModelCatalogCommand;
use super::entity::GenerationModel;
use super::events::{
    GenerationModelAdded, GenerationModelCatalogCreated, GenerationModelCatalogEvent,
    GenerationModelRemoved, GenerationModelUpdated,
};

const GENERATION_MODEL_CATALOG_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a002");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModelCatalog {
    state: CatalogState<GenerationModel>,
}

impl GenerationModelCatalog {
    pub fn singleton_id() -> Uuid {
        GENERATION_MODEL_CATALOG_ID
    }

    pub fn models(&self) -> &[GenerationModel] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

impl Aggregate for GenerationModelCatalog {
    type Event = GenerationModelCatalogEvent;
    type Command = GenerationModelCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "generation_model_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::GenerationModelCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::GenerationModelAdded(e) => {
                self.state.push(GenerationModel {
                    generation_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                });
            }
            Self::Event::GenerationModelUpdated(e) => {
                self.state.update(GenerationModel {
                    generation_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                });
            }
            Self::Event::GenerationModelRemoved(e) => {
                self.state.remove(e.model_id);
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
            bootstrap.push(Self::Event::GenerationModelCatalogCreated(
                GenerationModelCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            GenerationModelCatalogCommand::AddGenerationModel(cmd) => {
                let candidate = GenerationModel {
                    generation_model_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    model: cmd.model,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::GenerationModelAdded(GenerationModelAdded {
                    model_id: candidate.generation_model_id,
                    kind: candidate.kind,
                    model: candidate.model,
                })]
            }
            GenerationModelCatalogCommand::UpdateGenerationModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                let candidate = GenerationModel {
                    generation_model_id: existing.generation_model_id,
                    kind: cmd.kind,
                    model: cmd.model,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), Some(existing.generation_model_id))?;
                vec![Self::Event::GenerationModelUpdated(
                    GenerationModelUpdated {
                        model_id: candidate.generation_model_id,
                        kind: candidate.kind,
                        model: candidate.model,
                    },
                )]
            }
            GenerationModelCatalogCommand::RemoveGenerationModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                vec![Self::Event::GenerationModelRemoved(
                    GenerationModelRemoved {
                        model_id: existing.generation_model_id,
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
                (None, Self::Event::GenerationModelCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::GenerationModelCatalogCreated(_)) | (None, _) => {
                    return None
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<GenerationModelCatalog, ()> for GenerationModelCatalogEvent {}
