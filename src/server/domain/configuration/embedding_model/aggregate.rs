use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::event_sourcing::Aggregate;

use super::commands::EmbeddingModelCatalogCommand;
use super::entity::EmbeddingModel;
use super::events::{
    EmbeddingModelAdded, EmbeddingModelCatalogCreated, EmbeddingModelCatalogEvent,
    EmbeddingModelRemoved, EmbeddingModelUpdated,
};
use super::exceptions::EmbeddingModelCatalogError;
use crate::server::domain::configuration::kinds::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelCatalog {
    pub catalog_id: Uuid,
    pub models: Vec<EmbeddingModel>,
}

impl EmbeddingModelCatalog {
    pub fn singleton_id() -> Uuid {
        Uuid::nil()
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            catalog_id,
            models: Vec::new(),
        }
    }

    fn find(&self, model_id: Uuid) -> Result<&EmbeddingModel, EmbeddingModelCatalogError> {
        self.models
            .iter()
            .find(|m| m.embedding_model_id == model_id)
            .ok_or(EmbeddingModelCatalogError::NotFound)
    }

    fn ensure_unique(
        &self,
        kind: AiProviderKind,
        model: &str,
        excluding: Option<Uuid>,
    ) -> Result<(), EmbeddingModelCatalogError> {
        if self
            .models
            .iter()
            .any(|m| m.kind == kind && m.model == model && Some(m.embedding_model_id) != excluding)
        {
            return Err(EmbeddingModelCatalogError::ValidationError(format!(
                "Embedding model {model} already exists for {}",
                kind.as_str()
            )));
        }
        Ok(())
    }
}

impl Aggregate for EmbeddingModelCatalog {
    type Event = EmbeddingModelCatalogEvent;
    type Command = EmbeddingModelCatalogCommand;
    type Error = EmbeddingModelCatalogError;

    fn aggregate_type() -> &'static str {
        "embedding_model_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::EmbeddingModelCatalogCreated(e) => {
                *self = Self::empty(e.catalog_id);
            }
            Self::Event::EmbeddingModelAdded(e) => {
                self.models.push(EmbeddingModel {
                    embedding_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                    dimensions: e.dimensions,
                });
            }
            Self::Event::EmbeddingModelUpdated(e) => {
                if let Some(m) = self
                    .models
                    .iter_mut()
                    .find(|m| m.embedding_model_id == e.model_id)
                {
                    m.kind = e.kind;
                    m.model = e.model.clone();
                    m.dimensions = e.dimensions;
                }
            }
            Self::Event::EmbeddingModelRemoved(e) => {
                self.models.retain(|m| m.embedding_model_id != e.model_id);
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
        let state = &owned_state;

        let mut events = match command {
            EmbeddingModelCatalogCommand::AddEmbeddingModel(cmd) => {
                validate_non_empty("embedding model", &cmd.model)?;
                validate_positive("embedding dimensions", cmd.dimensions)?;
                validate_model_id_format(cmd.kind, &cmd.model)?;
                state.ensure_unique(cmd.kind, &cmd.model, None)?;
                vec![Self::Event::EmbeddingModelAdded(EmbeddingModelAdded {
                    model_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    model: cmd.model,
                    dimensions: cmd.dimensions,
                })]
            }
            EmbeddingModelCatalogCommand::UpdateEmbeddingModel(cmd) => {
                let existing = state.find(cmd.model_id)?;
                validate_non_empty("embedding model", &cmd.model)?;
                validate_positive("embedding dimensions", cmd.dimensions)?;
                validate_model_id_format(cmd.kind, &cmd.model)?;
                state.ensure_unique(cmd.kind, &cmd.model, Some(existing.embedding_model_id))?;
                vec![Self::Event::EmbeddingModelUpdated(EmbeddingModelUpdated {
                    model_id: existing.embedding_model_id,
                    kind: cmd.kind,
                    model: cmd.model,
                    dimensions: cmd.dimensions,
                })]
            }
            EmbeddingModelCatalogCommand::RemoveEmbeddingModel(cmd) => {
                let existing = state.find(cmd.model_id)?;
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

fn validate_non_empty(field: &str, value: &str) -> Result<(), EmbeddingModelCatalogError> {
    if value.trim().is_empty() {
        return Err(EmbeddingModelCatalogError::ValidationError(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(())
}

fn validate_positive(field: &str, value: u32) -> Result<(), EmbeddingModelCatalogError> {
    if value == 0 {
        return Err(EmbeddingModelCatalogError::ValidationError(format!(
            "{field} must be greater than zero"
        )));
    }
    Ok(())
}

fn validate_model_id_format(
    kind: AiProviderKind,
    model_id: &str,
) -> Result<(), EmbeddingModelCatalogError> {
    if !kind.model_id_well_formed(model_id) {
        return Err(EmbeddingModelCatalogError::ValidationError(format!(
            "model id '{model_id}' is not well-formed for provider kind {}",
            kind.as_str()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::commands::AddEmbeddingModel;
    use super::*;
    use crate::server::domain::configuration::kinds::AiProviderKind;

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
        assert_eq!(state.models.len(), 1);
    }

    #[test]
    fn duplicate_kind_and_name_rejected() {
        let mut events =
            EmbeddingModelCatalog::handle_command(None, add_cmd("@cf/baai/bge-base-en-v1.5", 768))
                .unwrap();
        let state = EmbeddingModelCatalog::from_events(&events).unwrap();
        let err = EmbeddingModelCatalog::handle_command(
            Some(&state),
            add_cmd("@cf/baai/bge-base-en-v1.5", 768),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            EmbeddingModelCatalogError::ValidationError(_)
        ));
        events.clear();
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
        assert!(matches!(
            err,
            EmbeddingModelCatalogError::ValidationError(_)
        ));
    }
}
