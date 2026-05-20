use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

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

#[cfg(test)]
mod tests {
    use super::super::commands::{
        AddGenerationModel, RemoveGenerationModel, UpdateGenerationModel,
    };
    use super::*;
    use crate::shared::reference_data::AiProviderKind;

    fn add(kind: AiProviderKind, model: &str) -> GenerationModelCatalogCommand {
        GenerationModelCatalogCommand::AddGenerationModel(AddGenerationModel {
            kind,
            model: model.into(),
        })
    }

    fn seeded() -> (GenerationModelCatalog, Vec<GenerationModelCatalogEvent>) {
        let events =
            GenerationModelCatalog::handle_command(None, add(AiProviderKind::Ollama, "llama3"))
                .expect("seed");
        let state = GenerationModelCatalog::from_events(&events).expect("seed state");
        (state, events)
    }

    #[test]
    fn first_add_emits_catalog_created_then_added() {
        let events =
            GenerationModelCatalog::handle_command(None, add(AiProviderKind::Ollama, "llama3"))
                .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            GenerationModelCatalogEvent::GenerationModelCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            GenerationModelCatalogEvent::GenerationModelAdded(_)
        ));
    }

    #[test]
    fn replay_builds_state_with_one_entry() {
        let (state, _) = seeded();
        assert_eq!(state.models().len(), 1);
        assert_eq!(state.models()[0].model, "llama3");
        assert_eq!(state.models()[0].kind, AiProviderKind::Ollama);
    }

    #[test]
    fn duplicate_kind_and_model_rejected() {
        let (state, _) = seeded();
        let err = GenerationModelCatalog::handle_command(
            Some(&state),
            add(AiProviderKind::Ollama, "llama3"),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn same_model_under_different_kind_is_allowed() {
        let (state, mut events) = seeded();
        let extra = GenerationModelCatalog::handle_command(
            Some(&state),
            add(AiProviderKind::Cloudflare, "@cf/meta/llama-3"),
        )
        .unwrap();
        events.extend(extra);
        let state = GenerationModelCatalog::from_events(&events).unwrap();
        assert_eq!(state.models().len(), 2);
    }

    #[test]
    fn invalid_cloudflare_model_id_rejected_at_command_layer() {
        let err = GenerationModelCatalog::handle_command(
            None,
            add(AiProviderKind::Cloudflare, "no-prefix-here"),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_rewrites_entry_in_place() {
        let (state, mut events) = seeded();
        let id = state.models()[0].generation_model_id;
        let upd = GenerationModelCatalog::handle_command(
            Some(&state),
            GenerationModelCatalogCommand::UpdateGenerationModel(UpdateGenerationModel {
                model_id: id,
                kind: AiProviderKind::Cloudflare,
                model: "@cf/meta/llama-3".into(),
            }),
        )
        .unwrap();
        events.extend(upd);
        let updated = GenerationModelCatalog::from_events(&events).unwrap();
        assert_eq!(updated.models().len(), 1);
        let e = &updated.models()[0];
        assert_eq!(e.generation_model_id, id);
        assert_eq!(e.kind, AiProviderKind::Cloudflare);
        assert_eq!(e.model, "@cf/meta/llama-3");
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = GenerationModelCatalog::handle_command(
            Some(&state),
            GenerationModelCatalogCommand::UpdateGenerationModel(UpdateGenerationModel {
                model_id: Uuid::new_v4(),
                kind: AiProviderKind::Ollama,
                model: "mistral".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn update_validates_new_model_id_format() {
        let (state, _) = seeded();
        let id = state.models()[0].generation_model_id;
        let err = GenerationModelCatalog::handle_command(
            Some(&state),
            GenerationModelCatalogCommand::UpdateGenerationModel(UpdateGenerationModel {
                model_id: id,
                kind: AiProviderKind::Cloudflare,
                model: "no-prefix".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_into_another_entrys_natural_key_rejected() {
        let (state, mut events) = seeded();
        let extra = GenerationModelCatalog::handle_command(
            Some(&state),
            add(AiProviderKind::Ollama, "mistral"),
        )
        .unwrap();
        events.extend(extra);
        let state = GenerationModelCatalog::from_events(&events).unwrap();
        let mistral_id = state
            .models()
            .iter()
            .find(|m| m.model == "mistral")
            .unwrap()
            .generation_model_id;
        let err = GenerationModelCatalog::handle_command(
            Some(&state),
            GenerationModelCatalogCommand::UpdateGenerationModel(UpdateGenerationModel {
                model_id: mistral_id,
                kind: AiProviderKind::Ollama,
                model: "llama3".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn remove_deletes_entry() {
        let (state, mut events) = seeded();
        let id = state.models()[0].generation_model_id;
        events.extend(
            GenerationModelCatalog::handle_command(
                Some(&state),
                GenerationModelCatalogCommand::RemoveGenerationModel(RemoveGenerationModel {
                    model_id: id,
                }),
            )
            .unwrap(),
        );
        let after = GenerationModelCatalog::from_events(&events).unwrap();
        assert!(after.models().is_empty());
    }

    #[test]
    fn remove_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = GenerationModelCatalog::handle_command(
            Some(&state),
            GenerationModelCatalogCommand::RemoveGenerationModel(RemoveGenerationModel {
                model_id: Uuid::new_v4(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn replay_with_event_before_creation_returns_none() {
        let events = vec![GenerationModelCatalogEvent::GenerationModelRemoved(
            GenerationModelRemoved {
                model_id: Uuid::new_v4(),
            },
        )];
        assert!(GenerationModelCatalog::from_events(&events).is_none());
    }

    #[test]
    fn aggregate_type_is_stable_identifier() {
        assert_eq!(
            GenerationModelCatalog::aggregate_type(),
            "generation_model_catalog",
        );
    }
}
