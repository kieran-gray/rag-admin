use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::{
    AddPipelineConfiguration, EmbeddingModelRef, PipelineConfigurationCatalogCommand,
    UpdatePipelineConfiguration, VectorIndexRef,
};
use super::entity::PipelineConfiguration;
use super::events::{
    PipelineConfigurationAdded, PipelineConfigurationCatalogCreated,
    PipelineConfigurationCatalogEvent, PipelineConfigurationRemoved, PipelineConfigurationUpdated,
};

const PIPELINE_CONFIGURATION_CATALOG_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a006");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfigurationCatalog {
    state: CatalogState<PipelineConfiguration>,
}

impl PipelineConfigurationCatalog {
    pub fn singleton_id() -> Uuid {
        PIPELINE_CONFIGURATION_CATALOG_ID
    }

    pub fn entries(&self) -> &[PipelineConfiguration] {
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

fn entry_from_add(
    id: Uuid,
    cmd: AddPipelineConfiguration,
) -> Result<PipelineConfiguration, CatalogError> {
    let dimensions = ensure_dimensions_match(&cmd.embedding_model, &cmd.vector_index)?;
    Ok(PipelineConfiguration {
        pipeline_configuration_id: id,
        name: cmd.name,
        embedding_model_id: cmd.embedding_model.embedding_model_id,
        generation_model_id: cmd.generation_model.generation_model_id,
        vector_index_id: cmd.vector_index.vector_index_id,
        dimensions,
    })
}

fn entry_from_update(
    id: Uuid,
    cmd: UpdatePipelineConfiguration,
) -> Result<PipelineConfiguration, CatalogError> {
    let dimensions = ensure_dimensions_match(&cmd.embedding_model, &cmd.vector_index)?;
    Ok(PipelineConfiguration {
        pipeline_configuration_id: id,
        name: cmd.name,
        embedding_model_id: cmd.embedding_model.embedding_model_id,
        generation_model_id: cmd.generation_model.generation_model_id,
        vector_index_id: cmd.vector_index.vector_index_id,
        dimensions,
    })
}

impl Aggregate for PipelineConfigurationCatalog {
    type Event = PipelineConfigurationCatalogEvent;
    type Command = PipelineConfigurationCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "pipeline_configuration_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::PipelineConfigurationCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::PipelineConfigurationAdded(e) => {
                self.state.push(PipelineConfiguration {
                    pipeline_configuration_id: e.pipeline_configuration_id,
                    name: e.name.clone(),
                    embedding_model_id: e.embedding_model_id,
                    generation_model_id: e.generation_model_id,
                    vector_index_id: e.vector_index_id,
                    dimensions: e.dimensions,
                });
            }
            Self::Event::PipelineConfigurationUpdated(e) => {
                self.state.update(PipelineConfiguration {
                    pipeline_configuration_id: e.pipeline_configuration_id,
                    name: e.name.clone(),
                    embedding_model_id: e.embedding_model_id,
                    generation_model_id: e.generation_model_id,
                    vector_index_id: e.vector_index_id,
                    dimensions: e.dimensions,
                });
            }
            Self::Event::PipelineConfigurationRemoved(e) => {
                self.state.remove(e.pipeline_configuration_id);
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
            bootstrap.push(Self::Event::PipelineConfigurationCatalogCreated(
                PipelineConfigurationCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            PipelineConfigurationCatalogCommand::AddPipelineConfiguration(cmd) => {
                let candidate = entry_from_add(Uuid::new_v4(), cmd)?;
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::PipelineConfigurationAdded(
                    PipelineConfigurationAdded {
                        pipeline_configuration_id: candidate.pipeline_configuration_id,
                        name: candidate.name,
                        embedding_model_id: candidate.embedding_model_id,
                        generation_model_id: candidate.generation_model_id,
                        vector_index_id: candidate.vector_index_id,
                        dimensions: candidate.dimensions,
                    },
                )]
            }
            PipelineConfigurationCatalogCommand::UpdatePipelineConfiguration(cmd) => {
                let existing = owned_state.state.find(cmd.pipeline_configuration_id)?;
                let candidate = entry_from_update(existing.pipeline_configuration_id, cmd)?;
                candidate.validate()?;
                owned_state.state.ensure_unique(
                    &candidate.natural_key(),
                    Some(existing.pipeline_configuration_id),
                )?;
                vec![Self::Event::PipelineConfigurationUpdated(
                    PipelineConfigurationUpdated {
                        pipeline_configuration_id: candidate.pipeline_configuration_id,
                        name: candidate.name,
                        embedding_model_id: candidate.embedding_model_id,
                        generation_model_id: candidate.generation_model_id,
                        vector_index_id: candidate.vector_index_id,
                        dimensions: candidate.dimensions,
                    },
                )]
            }
            PipelineConfigurationCatalogCommand::RemovePipelineConfiguration(cmd) => {
                let existing = owned_state.state.find(cmd.pipeline_configuration_id)?;
                vec![Self::Event::PipelineConfigurationRemoved(
                    PipelineConfigurationRemoved {
                        pipeline_configuration_id: existing.pipeline_configuration_id,
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
                (None, Self::Event::PipelineConfigurationCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::PipelineConfigurationCatalogCreated(_)) | (None, _) => {
                    return None
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<PipelineConfigurationCatalog, ()> for PipelineConfigurationCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::{GenerationModelRef, RemovePipelineConfiguration};
    use super::*;

    fn add(name: &str, em_dim: u32, vi_dim: u32) -> PipelineConfigurationCatalogCommand {
        PipelineConfigurationCatalogCommand::AddPipelineConfiguration(AddPipelineConfiguration {
            name: name.into(),
            embedding_model: EmbeddingModelRef {
                embedding_model_id: Uuid::new_v4(),
                dimensions: em_dim,
            },
            generation_model: GenerationModelRef {
                generation_model_id: Uuid::new_v4(),
            },
            vector_index: VectorIndexRef {
                vector_index_id: Uuid::new_v4(),
                dimensions: vi_dim,
            },
        })
    }

    #[test]
    fn first_add_bootstraps_catalog() {
        let events =
            PipelineConfigurationCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        assert!(matches!(
            events[0],
            PipelineConfigurationCatalogEvent::PipelineConfigurationCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            PipelineConfigurationCatalogEvent::PipelineConfigurationAdded(_)
        ));
    }

    #[test]
    fn dimension_mismatch_rejected() {
        let err =
            PipelineConfigurationCatalog::handle_command(None, add("bad", 1024, 768)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn duplicate_name_rejected() {
        let events =
            PipelineConfigurationCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        let state = PipelineConfigurationCatalog::from_events(&events).unwrap();
        let err =
            PipelineConfigurationCatalog::handle_command(Some(&state), add("default", 1024, 1024))
                .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn empty_name_rejected() {
        let err =
            PipelineConfigurationCatalog::handle_command(None, add("  ", 1024, 1024)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn remove_drops_entry() {
        let events =
            PipelineConfigurationCatalog::handle_command(None, add("default", 1024, 1024)).unwrap();
        let state_after_add = PipelineConfigurationCatalog::from_events(&events).unwrap();
        let id = state_after_add.entries()[0].pipeline_configuration_id;
        let remove_events = PipelineConfigurationCatalog::handle_command(
            Some(&state_after_add),
            PipelineConfigurationCatalogCommand::RemovePipelineConfiguration(
                RemovePipelineConfiguration {
                    pipeline_configuration_id: id,
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            remove_events[0],
            PipelineConfigurationCatalogEvent::PipelineConfigurationRemoved(_)
        ));
    }
}
