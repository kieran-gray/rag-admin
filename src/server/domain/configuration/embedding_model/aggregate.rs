use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use crate::server::event_sourcing::Aggregate;

use super::commands::EmbeddingModelCatalogCommand;
use super::entity::EmbeddingModel;
use super::events::{
    EmbeddingModelAdded, EmbeddingModelCatalogCreated, EmbeddingModelCatalogEvent,
    EmbeddingModelRemoved, EmbeddingModelUpdated,
};

const EMBEDDING_MODEL_CATALOG_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a001");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelCatalog {
    state: CatalogState<EmbeddingModel>,
}

impl EmbeddingModelCatalog {
    pub fn singleton_id() -> Uuid {
        EMBEDDING_MODEL_CATALOG_ID
    }

    pub fn models(&self) -> &[EmbeddingModel] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

impl Aggregate for EmbeddingModelCatalog {
    type Event = EmbeddingModelCatalogEvent;
    type Command = EmbeddingModelCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "embedding_model_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::EmbeddingModelCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::EmbeddingModelAdded(e) => {
                self.state.push(EmbeddingModel {
                    embedding_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::EmbeddingModelUpdated(e) => {
                self.state.update(EmbeddingModel {
                    embedding_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::EmbeddingModelRemoved(e) => {
                self.state.remove(e.model_id);
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
                bootstrap.push(Self::Event::EmbeddingModelCatalogCreated(
                    EmbeddingModelCatalogCreated {
                        catalog_id: Self::singleton_id(),
                    },
                ));
                Self::empty(Self::singleton_id())
            }
        };

        let mut events = match command {
            EmbeddingModelCatalogCommand::AddEmbeddingModel(cmd) => {
                let candidate = EmbeddingModel {
                    embedding_model_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    model: cmd.model,
                    dimensions: cmd.dimensions,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::EmbeddingModelAdded(EmbeddingModelAdded {
                    model_id: candidate.embedding_model_id,
                    kind: candidate.kind,
                    model: candidate.model,
                    dimensions: candidate.dimensions,
                })]
            }
            EmbeddingModelCatalogCommand::UpdateEmbeddingModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                let candidate = EmbeddingModel {
                    embedding_model_id: existing.embedding_model_id,
                    kind: cmd.kind,
                    model: cmd.model,
                    dimensions: cmd.dimensions,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), Some(existing.embedding_model_id))?;
                vec![Self::Event::EmbeddingModelUpdated(EmbeddingModelUpdated {
                    model_id: candidate.embedding_model_id,
                    kind: candidate.kind,
                    model: candidate.model,
                    dimensions: candidate.dimensions,
                })]
            }
            EmbeddingModelCatalogCommand::RemoveEmbeddingModel(cmd) => {
                let existing = owned_state.state.find(cmd.model_id)?;
                vec![Self::Event::EmbeddingModelRemoved(EmbeddingModelRemoved {
                    model_id: existing.embedding_model_id,
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
                (None, Self::Event::EmbeddingModelCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::EmbeddingModelCatalogCreated(_)) => return None,
                (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::super::commands::AddEmbeddingModel;
    use super::*;
    use crate::catalog::AiProviderKind;

    fn add_cmd(model: &str, dim: u32) -> EmbeddingModelCatalogCommand {
        EmbeddingModelCatalogCommand::AddEmbeddingModel(AddEmbeddingModel {
            kind: AiProviderKind::Cloudflare,
            model: model.into(),
            dimensions: dim,
        })
    }

    #[test]
    fn first_add_bootstraps_catalog() {
        let events =
            EmbeddingModelCatalog::handle_command(None, add_cmd("@cf/baai/bge-base-en-v1.5", 768))
                .unwrap();
        assert!(matches!(
            events[0],
            EmbeddingModelCatalogEvent::EmbeddingModelCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            EmbeddingModelCatalogEvent::EmbeddingModelAdded(_)
        ));
        let state = EmbeddingModelCatalog::from_events(&events).unwrap();
        assert_eq!(state.models().len(), 1);
    }

    #[test]
    fn duplicate_kind_and_name_rejected() {
        let events =
            EmbeddingModelCatalog::handle_command(None, add_cmd("@cf/baai/bge-base-en-v1.5", 768))
                .unwrap();
        let state = EmbeddingModelCatalog::from_events(&events).unwrap();
        let err = EmbeddingModelCatalog::handle_command(
            Some(&state),
            add_cmd("@cf/baai/bge-base-en-v1.5", 768),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn malformed_model_id_for_kind_rejected() {
        let err = EmbeddingModelCatalog::handle_command(
            Some(&EmbeddingModelCatalog::empty(
                EmbeddingModelCatalog::singleton_id(),
            )),
            add_cmd("text-embedding-3-small", 1536),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }
}
