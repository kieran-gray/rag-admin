use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError, CatalogState};
use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::ChunkingConfigurationCatalogCommand;
use super::entity::ChunkingConfiguration;
use super::events::{
    ChunkingConfigurationAdded, ChunkingConfigurationCatalogCreated,
    ChunkingConfigurationCatalogEvent, ChunkingConfigurationRemoved, ChunkingConfigurationUpdated,
};

const CHUNKING_CONFIGURATION_CATALOG_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a005");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfigurationCatalog {
    state: CatalogState<ChunkingConfiguration>,
}

impl ChunkingConfigurationCatalog {
    pub fn singleton_id() -> Uuid {
        CHUNKING_CONFIGURATION_CATALOG_ID
    }

    pub fn entries(&self) -> &[ChunkingConfiguration] {
        &self.state.entries
    }

    fn empty(catalog_id: Uuid) -> Self {
        Self {
            state: CatalogState::empty(catalog_id),
        }
    }
}

impl Aggregate for ChunkingConfigurationCatalog {
    type Event = ChunkingConfigurationCatalogEvent;
    type Command = ChunkingConfigurationCatalogCommand;
    type Error = CatalogError;

    fn aggregate_type() -> &'static str {
        "chunking_configuration_catalog"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::ChunkingConfigurationCatalogCreated(e) => {
                self.state = CatalogState::empty(e.catalog_id);
            }
            Self::Event::ChunkingConfigurationAdded(e) => {
                self.state.push(ChunkingConfiguration {
                    chunking_configuration_id: e.chunking_configuration_id,
                    name: e.name.clone(),
                    config: e.config,
                });
            }
            Self::Event::ChunkingConfigurationUpdated(e) => {
                self.state.update(ChunkingConfiguration {
                    chunking_configuration_id: e.chunking_configuration_id,
                    name: e.name.clone(),
                    config: e.config,
                });
            }
            Self::Event::ChunkingConfigurationRemoved(e) => {
                self.state.remove(e.chunking_configuration_id);
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
            bootstrap.push(Self::Event::ChunkingConfigurationCatalogCreated(
                ChunkingConfigurationCatalogCreated {
                    catalog_id: Self::singleton_id(),
                },
            ));
            Self::empty(Self::singleton_id())
        };

        let mut events = match command {
            ChunkingConfigurationCatalogCommand::AddChunkingConfiguration(cmd) => {
                let candidate = ChunkingConfiguration {
                    chunking_configuration_id: Uuid::new_v4(),
                    name: cmd.name,
                    config: cmd.config,
                };
                candidate.validate()?;
                owned_state
                    .state
                    .ensure_unique(&candidate.natural_key(), None)?;
                vec![Self::Event::ChunkingConfigurationAdded(
                    ChunkingConfigurationAdded {
                        chunking_configuration_id: candidate.chunking_configuration_id,
                        name: candidate.name,
                        config: candidate.config,
                    },
                )]
            }
            ChunkingConfigurationCatalogCommand::UpdateChunkingConfiguration(cmd) => {
                let existing = owned_state.state.find(cmd.chunking_configuration_id)?;
                let candidate = ChunkingConfiguration {
                    chunking_configuration_id: existing.chunking_configuration_id,
                    name: cmd.name,
                    config: cmd.config,
                };
                candidate.validate()?;
                owned_state.state.ensure_unique(
                    &candidate.natural_key(),
                    Some(existing.chunking_configuration_id),
                )?;
                vec![Self::Event::ChunkingConfigurationUpdated(
                    ChunkingConfigurationUpdated {
                        chunking_configuration_id: candidate.chunking_configuration_id,
                        name: candidate.name,
                        config: candidate.config,
                    },
                )]
            }
            ChunkingConfigurationCatalogCommand::RemoveChunkingConfiguration(cmd) => {
                let existing = owned_state.state.find(cmd.chunking_configuration_id)?;
                vec![Self::Event::ChunkingConfigurationRemoved(
                    ChunkingConfigurationRemoved {
                        chunking_configuration_id: existing.chunking_configuration_id,
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
                (None, Self::Event::ChunkingConfigurationCatalogCreated(e)) => {
                    state = Some(Self::empty(e.catalog_id));
                }
                (Some(_), Self::Event::ChunkingConfigurationCatalogCreated(_)) | (None, _) => {
                    return None
                }
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

impl HasPolicies<ChunkingConfigurationCatalog, ()> for ChunkingConfigurationCatalogEvent {}

#[cfg(test)]
mod tests {
    use super::super::commands::{
        AddChunkingConfiguration, RemoveChunkingConfiguration, UpdateChunkingConfiguration,
    };
    use super::*;
    use crate::shared::{ChunkingConfig, SectionChunkingConfig};

    fn add_cmd(name: &str) -> ChunkingConfigurationCatalogCommand {
        ChunkingConfigurationCatalogCommand::AddChunkingConfiguration(AddChunkingConfiguration {
            name: name.into(),
            config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: 384,
            }),
        })
    }

    #[test]
    fn first_add_bootstraps_catalog() {
        let events =
            ChunkingConfigurationCatalog::handle_command(None, add_cmd("section-384")).unwrap();
        assert!(matches!(
            events[0],
            ChunkingConfigurationCatalogEvent::ChunkingConfigurationCatalogCreated(_)
        ));
        assert!(matches!(
            events[1],
            ChunkingConfigurationCatalogEvent::ChunkingConfigurationAdded(_)
        ));
        let state = ChunkingConfigurationCatalog::from_events(&events).unwrap();
        assert_eq!(state.entries().len(), 1);
    }

    #[test]
    fn duplicate_name_rejected() {
        let events =
            ChunkingConfigurationCatalog::handle_command(None, add_cmd("section-384")).unwrap();
        let state = ChunkingConfigurationCatalog::from_events(&events).unwrap();
        let err =
            ChunkingConfigurationCatalog::handle_command(Some(&state), add_cmd("section-384"))
                .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn empty_name_rejected() {
        let err = ChunkingConfigurationCatalog::handle_command(None, add_cmd("   ")).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_unknown_id_returns_not_found() {
        let events =
            ChunkingConfigurationCatalog::handle_command(None, add_cmd("section-384")).unwrap();
        let state = ChunkingConfigurationCatalog::from_events(&events).unwrap();
        let err = ChunkingConfigurationCatalog::handle_command(
            Some(&state),
            ChunkingConfigurationCatalogCommand::UpdateChunkingConfiguration(
                UpdateChunkingConfiguration {
                    chunking_configuration_id: Uuid::new_v4(),
                    name: "other".into(),
                    config: ChunkingConfig::Section(SectionChunkingConfig {
                        max_section_tokens: 256,
                    }),
                },
            ),
        )
        .unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn remove_drops_entry() {
        let events =
            ChunkingConfigurationCatalog::handle_command(None, add_cmd("section-384")).unwrap();
        let state_after_add = ChunkingConfigurationCatalog::from_events(&events).unwrap();
        let id = state_after_add.entries()[0].chunking_configuration_id;

        let remove_events = ChunkingConfigurationCatalog::handle_command(
            Some(&state_after_add),
            ChunkingConfigurationCatalogCommand::RemoveChunkingConfiguration(
                RemoveChunkingConfiguration {
                    chunking_configuration_id: id,
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            remove_events[0],
            ChunkingConfigurationCatalogEvent::ChunkingConfigurationRemoved(_)
        ));
    }
}
