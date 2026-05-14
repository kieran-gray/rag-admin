use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::kinds::AiProviderKind;
use crate::server::event_sourcing::Aggregate;

use super::commands::GenerationModelCatalogCommand;
use super::entity::GenerationModel;
use super::events::{
    GenerationModelAdded, GenerationModelCatalogCreated, GenerationModelCatalogEvent,
    GenerationModelRemoved, GenerationModelUpdated,
};
use super::exceptions::GenerationModelCatalogError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModelCatalog {
    pub catalog_id: Uuid,
    pub models: Vec<GenerationModel>,
}

impl GenerationModelCatalog {
    pub fn singleton_id() -> Uuid {
        Uuid::nil()
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            catalog_id,
            models: Vec::new(),
        }
    }

    fn find(&self, model_id: Uuid) -> Result<&GenerationModel, GenerationModelCatalogError> {
        self.models
            .iter()
            .find(|m| m.generation_model_id == model_id)
            .ok_or(GenerationModelCatalogError::NotFound)
    }

    fn ensure_unique(
        &self,
        kind: AiProviderKind,
        model: &str,
        excluding: Option<Uuid>,
    ) -> Result<(), GenerationModelCatalogError> {
        if self
            .models
            .iter()
            .any(|m| m.kind == kind && m.model == model && Some(m.generation_model_id) != excluding)
        {
            return Err(GenerationModelCatalogError::ValidationError(format!(
                "Generation model {model} already exists for {}",
                kind.as_str()
            )));
        }
        Ok(())
    }
}

impl Aggregate for GenerationModelCatalog {
    type Event = GenerationModelCatalogEvent;
    type Command = GenerationModelCatalogCommand;
    type Error = GenerationModelCatalogError;

    fn aggregate_type() -> &'static str {
        "generation_model_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::GenerationModelCatalogCreated(e) => {
                *self = Self::empty(e.catalog_id);
            }
            Self::Event::GenerationModelAdded(e) => {
                self.models.push(GenerationModel {
                    generation_model_id: e.model_id,
                    kind: e.kind,
                    model: e.model.clone(),
                });
            }
            Self::Event::GenerationModelUpdated(e) => {
                if let Some(m) = self
                    .models
                    .iter_mut()
                    .find(|m| m.generation_model_id == e.model_id)
                {
                    m.kind = e.kind;
                    m.model = e.model.clone();
                }
            }
            Self::Event::GenerationModelRemoved(e) => {
                self.models.retain(|m| m.generation_model_id != e.model_id);
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
                bootstrap.push(Self::Event::GenerationModelCatalogCreated(
                    GenerationModelCatalogCreated {
                        catalog_id: Self::singleton_id(),
                    },
                ));
                Self::empty(Self::singleton_id())
            }
        };
        let state = &owned_state;

        let mut events = match command {
            GenerationModelCatalogCommand::AddGenerationModel(cmd) => {
                validate_non_empty("generation model", &cmd.model)?;
                validate_model_id_format(cmd.kind, &cmd.model)?;
                state.ensure_unique(cmd.kind, &cmd.model, None)?;
                vec![Self::Event::GenerationModelAdded(GenerationModelAdded {
                    model_id: Uuid::new_v4(),
                    kind: cmd.kind,
                    model: cmd.model,
                })]
            }
            GenerationModelCatalogCommand::UpdateGenerationModel(cmd) => {
                let existing = state.find(cmd.model_id)?;
                validate_non_empty("generation model", &cmd.model)?;
                validate_model_id_format(cmd.kind, &cmd.model)?;
                state.ensure_unique(cmd.kind, &cmd.model, Some(existing.generation_model_id))?;
                vec![Self::Event::GenerationModelUpdated(
                    GenerationModelUpdated {
                        model_id: existing.generation_model_id,
                        kind: cmd.kind,
                        model: cmd.model,
                    },
                )]
            }
            GenerationModelCatalogCommand::RemoveGenerationModel(cmd) => {
                let existing = state.find(cmd.model_id)?;
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
                (Some(_), Self::Event::GenerationModelCatalogCreated(_)) => return None,
                (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), GenerationModelCatalogError> {
    if value.trim().is_empty() {
        return Err(GenerationModelCatalogError::ValidationError(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(())
}

fn validate_model_id_format(
    kind: AiProviderKind,
    model_id: &str,
) -> Result<(), GenerationModelCatalogError> {
    if !kind.model_id_well_formed(model_id) {
        return Err(GenerationModelCatalogError::ValidationError(format!(
            "model id '{model_id}' is not well-formed for provider kind {}",
            kind.as_str()
        )));
    }
    Ok(())
}
