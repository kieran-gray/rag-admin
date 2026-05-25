use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::RerankerModelCatalogCommand;
use super::entity::RerankerModel;
use super::events::{
    RerankerModelAdded, RerankerModelCatalogCreated, RerankerModelCatalogEvent,
    RerankerModelRemoved, RerankerModelUpdated,
};

const RERANKER_MODEL_CATALOG_ID: Uuid = uuid::uuid!("c4a8f6e2-9b71-4c8d-b143-7f1c9f12d8a4");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankerModelCatalog {
    state: CatalogState<RerankerModel>,
}

impl RerankerModelCatalog {
    pub fn singleton_id() -> Uuid {
        RERANKER_MODEL_CATALOG_ID
    }

    pub fn models(&self) -> &[RerankerModel] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

impl Aggregate for RerankerModelCatalog {
    type Event = RerankerModelCatalogEvent;
    type Command = RerankerModelCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "reranker_model_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::RerankerModelCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::RerankerModelAdded(e) => {
                self.state.push(RerankerModel {
                    reranker_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                });
            }
            Self::Event::RerankerModelUpdated(e) => {
                self.state.update(RerankerModel {
                    reranker_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                });
            }
            Self::Event::RerankerModelRemoved(e) => {
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
            bootstrap.push(Self::Event::RerankerModelCatalogCreated(
                RerankerModelCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            RerankerModelCatalogCommand::AddRerankerModel(cmd) => {
                let candidate = RerankerModel {
                    reranker_model_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    model: cmd.model,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::RerankerModelAdded(RerankerModelAdded {
                    model_id: candidate.reranker_model_id,
                    kind: candidate.kind,
                    model: candidate.model,
                })]
            }
            RerankerModelCatalogCommand::UpdateRerankerModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                let candidate = RerankerModel {
                    reranker_model_id: existing.reranker_model_id,
                    kind: cmd.kind,
                    model: cmd.model,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), Some(existing.reranker_model_id))?;
                vec![Self::Event::RerankerModelUpdated(RerankerModelUpdated {
                    model_id: candidate.reranker_model_id,
                    kind: candidate.kind,
                    model: candidate.model,
                })]
            }
            RerankerModelCatalogCommand::RemoveRerankerModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                vec![Self::Event::RerankerModelRemoved(RerankerModelRemoved {
                    model_id: existing.reranker_model_id,
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
                (None, Self::Event::RerankerModelCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::RerankerModelCatalogCreated(_)) | (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<RerankerModelCatalog, ()> for RerankerModelCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::{AddRerankerModel, RemoveRerankerModel, UpdateRerankerModel};
    use super::*;
    use crate::shared::reference_data::AiProviderKind;

    fn add(kind: AiProviderKind, model: &str) -> RerankerModelCatalogCommand {
        RerankerModelCatalogCommand::AddRerankerModel(AddRerankerModel {
            kind,
            model: model.into(),
        })
    }

    fn seeded() -> (RerankerModelCatalog, Vec<RerankerModelCatalogEvent>) {
        let events = RerankerModelCatalog::handle_command(
            None,
            add(AiProviderKind::LlamaServer, "qwen3-reranker-4b"),
        )
        .expect("seed");
        let state = RerankerModelCatalog::from_events(&events).expect("seed state");
        (state, events)
    }

    #[test]
    fn first_add_emits_catalog_created_then_added() {
        let events = RerankerModelCatalog::handle_command(
            None,
            add(AiProviderKind::LlamaServer, "qwen3-reranker-4b"),
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            RerankerModelCatalogEvent::RerankerModelCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            RerankerModelCatalogEvent::RerankerModelAdded(_)
        ));
    }

    #[test]
    fn replay_builds_state_with_one_entry() {
        let (state, _) = seeded();
        assert_eq!(state.models().len(), 1);
        assert_eq!(state.models()[0].model, "qwen3-reranker-4b");
        assert_eq!(state.models()[0].kind, AiProviderKind::LlamaServer);
    }

    #[test]
    fn duplicate_kind_and_model_rejected() {
        let (state, _) = seeded();
        let err = RerankerModelCatalog::handle_command(
            Some(&state),
            add(AiProviderKind::LlamaServer, "qwen3-reranker-4b"),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn same_model_under_different_kind_is_allowed() {
        let (state, mut events) = seeded();
        let extra = RerankerModelCatalog::handle_command(
            Some(&state),
            add(AiProviderKind::Cloudflare, "@cf/baai/bge-reranker-base"),
        )
        .unwrap();
        events.extend(extra);
        let state = RerankerModelCatalog::from_events(&events).unwrap();
        assert_eq!(state.models().len(), 2);
    }

    #[test]
    fn invalid_cloudflare_model_id_rejected_at_command_layer() {
        let err = RerankerModelCatalog::handle_command(
            None,
            add(AiProviderKind::Cloudflare, "no-prefix-here"),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_rewrites_entry_in_place() {
        let (state, mut events) = seeded();
        let id = state.models()[0].reranker_model_id;
        let upd = RerankerModelCatalog::handle_command(
            Some(&state),
            RerankerModelCatalogCommand::UpdateRerankerModel(UpdateRerankerModel {
                model_id: id,
                kind: AiProviderKind::Cloudflare,
                model: "@cf/baai/bge-reranker-base".into(),
            }),
        )
        .unwrap();
        events.extend(upd);
        let updated = RerankerModelCatalog::from_events(&events).unwrap();
        assert_eq!(updated.models().len(), 1);
        let e = &updated.models()[0];
        assert_eq!(e.reranker_model_id, id);
        assert_eq!(e.kind, AiProviderKind::Cloudflare);
        assert_eq!(e.model, "@cf/baai/bge-reranker-base");
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = RerankerModelCatalog::handle_command(
            Some(&state),
            RerankerModelCatalogCommand::UpdateRerankerModel(UpdateRerankerModel {
                model_id: Uuid::new_v4(),
                kind: AiProviderKind::LlamaServer,
                model: "qwen3-reranker-8b".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn remove_deletes_entry() {
        let (state, mut events) = seeded();
        let id = state.models()[0].reranker_model_id;
        events.extend(
            RerankerModelCatalog::handle_command(
                Some(&state),
                RerankerModelCatalogCommand::RemoveRerankerModel(RemoveRerankerModel {
                    model_id: id,
                }),
            )
            .unwrap(),
        );
        let after = RerankerModelCatalog::from_events(&events).unwrap();
        assert!(after.models().is_empty());
    }

    #[test]
    fn remove_unknown_id_returns_not_found() {
        let (state, _) = seeded();
        let err = RerankerModelCatalog::handle_command(
            Some(&state),
            RerankerModelCatalogCommand::RemoveRerankerModel(RemoveRerankerModel {
                model_id: Uuid::new_v4(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn replay_with_event_before_creation_returns_none() {
        let events =
            vec![RerankerModelCatalogEvent::RerankerModelRemoved(
                RerankerModelRemoved {
                    model_id: Uuid::new_v4(),
                },
            )];
        assert!(RerankerModelCatalog::from_events(&events).is_none());
    }

    #[test]
    fn aggregate_type_is_stable_identifier() {
        assert_eq!(
            RerankerModelCatalog::aggregate_type(),
            "reranker_model_catalog",
        );
    }
}
