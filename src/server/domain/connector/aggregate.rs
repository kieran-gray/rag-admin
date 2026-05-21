use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::policy::HasPolicies;
use event_sourcing::Aggregate;

use super::commands::ConnectorCommand;
use super::config::ConnectorConfig;
use super::events::{
    ConnectorConfigUpdated, ConnectorDefaultsSet, ConnectorEvent, ConnectorRegistered,
    ConnectorRenamed, ConnectorUnregistered,
};
use super::exceptions::ConnectorError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub connector_id: Uuid,
    pub name: String,
    pub config: ConnectorConfig,
    pub deleted: bool,
    pub default_index_profile_id: Option<Uuid>,
    pub default_chunking_configuration_id: Option<Uuid>,
    pub default_retrieval_profile_id: Option<Uuid>,
}

impl Aggregate for Connector {
    type Event = ConnectorEvent;
    type Command = ConnectorCommand;
    type Error = ConnectorError;

    fn aggregate_type() -> &'static str {
        "connector"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ConnectorEvent::ConnectorRegistered(_) => {}
            ConnectorEvent::ConnectorRenamed(e) => {
                self.name.clone_from(&e.name);
            }
            ConnectorEvent::ConnectorConfigUpdated(e) => {
                self.config.clone_from(&e.config);
            }
            ConnectorEvent::ConnectorUnregistered(_) => {
                self.deleted = true;
            }
            ConnectorEvent::ConnectorDefaultsSet(e) => {
                self.default_index_profile_id = e.index_profile_id;
                self.default_chunking_configuration_id = e.chunking_configuration_id;
                self.default_retrieval_profile_id = e.retrieval_profile_id;
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            ConnectorCommand::RegisterConnector(cmd) => {
                if state.is_some() {
                    return Err(ConnectorError::AlreadyExists);
                }
                validate_name(&cmd.name)?;
                Ok(vec![ConnectorEvent::ConnectorRegistered(
                    ConnectorRegistered {
                        connector_id: cmd.connector_id,
                        name: cmd.name,
                        config: cmd.config,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorCommand::RenameConnector(cmd) => {
                let connector = state.ok_or(ConnectorError::NotFound)?;
                if connector.deleted {
                    return Err(ConnectorError::AlreadyDeleted);
                }
                validate_name(&cmd.name)?;
                if connector.name == cmd.name {
                    return Ok(vec![]);
                }
                Ok(vec![ConnectorEvent::ConnectorRenamed(ConnectorRenamed {
                    name: cmd.name,
                    occurred_at: cmd.occurred_at,
                })])
            }
            ConnectorCommand::UpdateConnectorConfig(cmd) => {
                let connector = state.ok_or(ConnectorError::NotFound)?;
                if connector.deleted {
                    return Err(ConnectorError::AlreadyDeleted);
                }
                if connector.config.kind() != cmd.config.kind() {
                    return Err(ConnectorError::ValidationError(
                        "cannot change connector kind".into(),
                    ));
                }
                if connector.config == cmd.config {
                    return Ok(vec![]);
                }
                Ok(vec![ConnectorEvent::ConnectorConfigUpdated(
                    ConnectorConfigUpdated {
                        config: cmd.config,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorCommand::UnregisterConnector(cmd) => {
                let connector = state.ok_or(ConnectorError::NotFound)?;
                if connector.deleted {
                    return Err(ConnectorError::AlreadyDeleted);
                }
                Ok(vec![ConnectorEvent::ConnectorUnregistered(
                    ConnectorUnregistered {
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            ConnectorCommand::SetConnectorDefaults(cmd) => {
                let connector = state.ok_or(ConnectorError::NotFound)?;
                if connector.deleted {
                    return Err(ConnectorError::AlreadyDeleted);
                }
                if connector.default_index_profile_id == cmd.index_profile_id
                    && connector.default_chunking_configuration_id == cmd.chunking_configuration_id
                    && connector.default_retrieval_profile_id == cmd.retrieval_profile_id
                {
                    return Ok(vec![]);
                }
                Ok(vec![ConnectorEvent::ConnectorDefaultsSet(
                    ConnectorDefaultsSet {
                        index_profile_id: cmd.index_profile_id,
                        chunking_configuration_id: cmd.chunking_configuration_id,
                        retrieval_profile_id: cmd.retrieval_profile_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;

        for event in events {
            match (&mut state, event) {
                (None, ConnectorEvent::ConnectorRegistered(e)) => {
                    state = Some(Self {
                        connector_id: e.connector_id,
                        name: e.name.clone(),
                        config: e.config.clone(),
                        deleted: false,
                        default_index_profile_id: None,
                        default_chunking_configuration_id: None,
                        default_retrieval_profile_id: None,
                    });
                }
                (Some(_), ConnectorEvent::ConnectorRegistered(_)) | (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }

        state
    }
}

impl HasPolicies<Connector, ()> for ConnectorEvent {}

fn validate_name(name: &str) -> Result<(), ConnectorError> {
    if name.trim().is_empty() {
        return Err(ConnectorError::ValidationError(
            "connector name must not be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::connector::config::SitemapConfig;

    fn now() -> crate::server::domain::shared::Timestamp {
        "2024-01-01T00:00:00Z".into()
    }

    fn sample_config() -> ConnectorConfig {
        ConnectorConfig::Sitemap(SitemapConfig {
            url: "https://example.com/sitemap.xml".into(),
            include_patterns: vec![],
            exclude_patterns: vec![],
        })
    }

    fn register(id: Uuid, name: &str) -> ConnectorCommand {
        ConnectorCommand::RegisterConnector(super::super::commands::RegisterConnector {
            connector_id: id,
            name: name.into(),
            config: sample_config(),
            occurred_at: now(),
        })
    }

    #[test]
    fn register_emits_registered_event() {
        let id = Uuid::new_v4();
        let events = Connector::handle_command(None, register(id, "my-blog")).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ConnectorEvent::ConnectorRegistered(_)));
    }

    #[test]
    fn register_with_empty_name_fails() {
        let id = Uuid::new_v4();
        let err = Connector::handle_command(None, register(id, "  ")).unwrap_err();
        assert!(matches!(err, ConnectorError::ValidationError(_)));
    }

    #[test]
    fn registering_twice_fails() {
        let id = Uuid::new_v4();
        let events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let err = Connector::handle_command(Some(&state), register(id, "a")).unwrap_err();
        assert!(matches!(err, ConnectorError::AlreadyExists));
    }

    #[test]
    fn rename_emits_event_and_applies() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "old")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::RenameConnector(super::super::commands::RenameConnector {
                connector_id: id,
                name: "new".into(),
                occurred_at: now(),
            }),
        )
        .unwrap();
        events.extend(new);
        let state = Connector::from_events(&events).unwrap();
        assert_eq!(state.name, "new");
    }

    #[test]
    fn rename_to_same_name_is_idempotent() {
        let id = Uuid::new_v4();
        let events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::RenameConnector(super::super::commands::RenameConnector {
                connector_id: id,
                name: "a".into(),
                occurred_at: now(),
            }),
        )
        .unwrap();
        assert!(new.is_empty());
    }

    #[test]
    fn update_config_changes_state() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let new_config = ConnectorConfig::Sitemap(SitemapConfig {
            url: "https://other.example/sitemap.xml".into(),
            include_patterns: vec!["/blog/".into()],
            exclude_patterns: vec![],
        });
        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::UpdateConnectorConfig(
                super::super::commands::UpdateConnectorConfig {
                    connector_id: id,
                    config: new_config.clone(),
                    occurred_at: now(),
                },
            ),
        )
        .unwrap();
        events.extend(new);
        let state = Connector::from_events(&events).unwrap();
        assert_eq!(state.config, new_config);
    }

    #[test]
    fn unregister_marks_deleted() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::UnregisterConnector(super::super::commands::UnregisterConnector {
                connector_id: id,
                occurred_at: now(),
            }),
        )
        .unwrap();
        events.extend(new);
        let state = Connector::from_events(&events).unwrap();
        assert!(state.deleted);
    }

    #[test]
    fn set_defaults_emits_event_and_applies() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        let index_profile_id = Uuid::new_v4();
        let chunking_id = Uuid::new_v4();
        let retrieval_profile_id = Uuid::new_v4();
        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::SetConnectorDefaults(super::super::commands::SetConnectorDefaults {
                connector_id: id,
                index_profile_id: Some(index_profile_id),
                chunking_configuration_id: Some(chunking_id),
                retrieval_profile_id: Some(retrieval_profile_id),
                occurred_at: now(),
            }),
        )
        .unwrap();
        assert_eq!(new.len(), 1);
        events.extend(new);
        let state = Connector::from_events(&events).unwrap();
        assert_eq!(state.default_index_profile_id, Some(index_profile_id));
        assert_eq!(state.default_chunking_configuration_id, Some(chunking_id));
        assert_eq!(
            state.default_retrieval_profile_id,
            Some(retrieval_profile_id)
        );
    }

    #[test]
    fn set_defaults_is_idempotent_when_unchanged() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let index_profile_id = Uuid::new_v4();
        let state = Connector::from_events(&events).unwrap();
        events.extend(
            Connector::handle_command(
                Some(&state),
                ConnectorCommand::SetConnectorDefaults(
                    super::super::commands::SetConnectorDefaults {
                        connector_id: id,
                        index_profile_id: Some(index_profile_id),
                        chunking_configuration_id: None,
                        retrieval_profile_id: None,
                        occurred_at: now(),
                    },
                ),
            )
            .unwrap(),
        );
        let state = Connector::from_events(&events).unwrap();

        let new = Connector::handle_command(
            Some(&state),
            ConnectorCommand::SetConnectorDefaults(super::super::commands::SetConnectorDefaults {
                connector_id: id,
                index_profile_id: Some(index_profile_id),
                chunking_configuration_id: None,
                retrieval_profile_id: None,
                occurred_at: now(),
            }),
        )
        .unwrap();
        assert!(new.is_empty());
    }

    #[test]
    fn set_defaults_can_clear_to_none() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let index_profile_id = Uuid::new_v4();
        let state = Connector::from_events(&events).unwrap();
        events.extend(
            Connector::handle_command(
                Some(&state),
                ConnectorCommand::SetConnectorDefaults(
                    super::super::commands::SetConnectorDefaults {
                        connector_id: id,
                        index_profile_id: Some(index_profile_id),
                        chunking_configuration_id: None,
                        retrieval_profile_id: None,
                        occurred_at: now(),
                    },
                ),
            )
            .unwrap(),
        );
        let state = Connector::from_events(&events).unwrap();
        events.extend(
            Connector::handle_command(
                Some(&state),
                ConnectorCommand::SetConnectorDefaults(
                    super::super::commands::SetConnectorDefaults {
                        connector_id: id,
                        index_profile_id: None,
                        chunking_configuration_id: None,
                        retrieval_profile_id: None,
                        occurred_at: now(),
                    },
                ),
            )
            .unwrap(),
        );
        let state = Connector::from_events(&events).unwrap();
        assert_eq!(state.default_index_profile_id, None);
    }

    #[test]
    fn double_unregister_fails() {
        let id = Uuid::new_v4();
        let mut events = Connector::handle_command(None, register(id, "a")).unwrap();
        let state = Connector::from_events(&events).unwrap();
        events.extend(
            Connector::handle_command(
                Some(&state),
                ConnectorCommand::UnregisterConnector(
                    super::super::commands::UnregisterConnector {
                        connector_id: id,
                        occurred_at: now(),
                    },
                ),
            )
            .unwrap(),
        );
        let state = Connector::from_events(&events).unwrap();
        let err = Connector::handle_command(
            Some(&state),
            ConnectorCommand::UnregisterConnector(super::super::commands::UnregisterConnector {
                connector_id: id,
                occurred_at: now(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorError::AlreadyDeleted));
    }
}
