use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::ConfigurationDefaultsCommand;
use super::events::{
    ConfigurationDefaultsEvent, DefaultChunkingConfigurationSet, DefaultIndexProfileSet,
    DefaultRetrievalProfileSet, DefaultSweepTemplateSet,
};
use super::exceptions::ConfigurationDefaultsError;

const CONFIGURATION_DEFAULTS_ID: Uuid = uuid::uuid!("8a17e2f1-3a9d-4e58-a9c9-0a0f61d4a004");

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigurationDefaults {
    pub chunking_configuration_id: Option<Uuid>,
    pub index_profile_id: Option<Uuid>,
    pub retrieval_profile_id: Option<Uuid>,
    pub sweep_template_id: Option<Uuid>,
}

impl ConfigurationDefaults {
    pub fn singleton_id() -> Uuid {
        CONFIGURATION_DEFAULTS_ID
    }
}

impl Aggregate for ConfigurationDefaults {
    type Event = ConfigurationDefaultsEvent;
    type Command = ConfigurationDefaultsCommand;
    type Error = ConfigurationDefaultsError;

    fn aggregate_type() -> &'static str {
        "configuration_defaults"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::DefaultChunkingConfigurationSet(e) => {
                self.chunking_configuration_id = Some(e.chunking_configuration_id);
            }
            Self::Event::DefaultIndexProfileSet(e) => {
                self.index_profile_id = Some(e.index_profile_id);
            }
            Self::Event::DefaultRetrievalProfileSet(e) => {
                self.retrieval_profile_id = Some(e.retrieval_profile_id);
            }
            Self::Event::DefaultSweepTemplateSet(e) => {
                self.sweep_template_id = Some(e.sweep_template_id);
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        let current = state.cloned().unwrap_or_default();

        match command {
            ConfigurationDefaultsCommand::SetDefaultChunkingConfiguration(cmd) => {
                if current.chunking_configuration_id == Some(cmd.chunking_configuration_id) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::DefaultChunkingConfigurationSet(
                    DefaultChunkingConfigurationSet {
                        chunking_configuration_id: cmd.chunking_configuration_id,
                    },
                )])
            }
            ConfigurationDefaultsCommand::SetDefaultIndexProfile(cmd) => {
                if current.index_profile_id == Some(cmd.index_profile_id) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::DefaultIndexProfileSet(
                    DefaultIndexProfileSet {
                        index_profile_id: cmd.index_profile_id,
                    },
                )])
            }
            ConfigurationDefaultsCommand::SetDefaultRetrievalProfile(cmd) => {
                if current.retrieval_profile_id == Some(cmd.retrieval_profile_id) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::DefaultRetrievalProfileSet(
                    DefaultRetrievalProfileSet {
                        retrieval_profile_id: cmd.retrieval_profile_id,
                    },
                )])
            }
            ConfigurationDefaultsCommand::SetDefaultSweepTemplate(cmd) => {
                if current.sweep_template_id == Some(cmd.sweep_template_id) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::DefaultSweepTemplateSet(
                    DefaultSweepTemplateSet {
                        sweep_template_id: cmd.sweep_template_id,
                    },
                )])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        if events.is_empty() {
            return None;
        }
        let mut state = Self::default();
        for event in events {
            state.apply(event);
        }
        Some(state)
    }
}

impl HasPolicies<ConfigurationDefaults, ()> for ConfigurationDefaultsEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::configuration::defaults::commands::{
        SetDefaultChunkingConfiguration, SetDefaultIndexProfile, SetDefaultRetrievalProfile,
        SetDefaultSweepTemplate,
    };

    #[test]
    fn first_set_emits_event() {
        let id = Uuid::new_v4();
        let events = ConfigurationDefaults::handle_command(
            None,
            ConfigurationDefaultsCommand::SetDefaultChunkingConfiguration(
                SetDefaultChunkingConfiguration {
                    chunking_configuration_id: id,
                },
            ),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let state = ConfigurationDefaults::from_events(&events).unwrap();
        assert_eq!(state.chunking_configuration_id, Some(id));
    }

    #[test]
    fn idempotent_when_unchanged() {
        let id = Uuid::new_v4();
        let state = ConfigurationDefaults {
            index_profile_id: Some(id),
            ..ConfigurationDefaults::default()
        };

        let events = ConfigurationDefaults::handle_command(
            Some(&state),
            ConfigurationDefaultsCommand::SetDefaultIndexProfile(SetDefaultIndexProfile {
                index_profile_id: id,
            }),
        )
        .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn changes_existing_default() {
        let old = Uuid::new_v4();
        let new = Uuid::new_v4();
        let state = ConfigurationDefaults {
            sweep_template_id: Some(old),
            ..ConfigurationDefaults::default()
        };

        let events = ConfigurationDefaults::handle_command(
            Some(&state),
            ConfigurationDefaultsCommand::SetDefaultSweepTemplate(SetDefaultSweepTemplate {
                sweep_template_id: new,
            }),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn retrieval_profile_default_round_trip() {
        let id = Uuid::new_v4();
        let events = ConfigurationDefaults::handle_command(
            None,
            ConfigurationDefaultsCommand::SetDefaultRetrievalProfile(SetDefaultRetrievalProfile {
                retrieval_profile_id: id,
            }),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let state = ConfigurationDefaults::from_events(&events).unwrap();
        assert_eq!(state.retrieval_profile_id, Some(id));
    }
}
