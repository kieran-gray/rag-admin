use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::event_sourcing::Aggregate;

use super::commands::SweepTemplateCommand;
use super::events::{
    SweepTemplateCreated, SweepTemplateDefaultSet, SweepTemplateDeleted, SweepTemplateEvent,
    SweepTemplateUpdated,
};
use super::exceptions::SweepTemplateError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepTemplate {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
    pub deleted: bool,
}

impl SweepTemplate {
    fn from_created(e: &SweepTemplateCreated) -> Self {
        Self {
            sweep_template_id: e.sweep_template_id,
            name: e.name.clone(),
            members: e.members.clone(),
            deleted: false,
        }
    }
}

impl Aggregate for SweepTemplate {
    type Event = SweepTemplateEvent;
    type Command = SweepTemplateCommand;
    type Error = SweepTemplateError;

    fn aggregate_type() -> &'static str {
        "sweep_template"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::SweepTemplateCreated(_) => {}
            Self::Event::SweepTemplateUpdated(e) => {
                self.name = e.name.clone();
                self.members = e.members.clone();
            }
            Self::Event::SweepTemplateDeleted(_) => {
                self.deleted = true;
            }
            Self::Event::SweepTemplateDefaultSet(_) => {}
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::CreateSweepTemplate(cmd) => {
                if state.is_some() {
                    return Err(SweepTemplateError::AlreadyExists);
                }
                validate_name(&cmd.name)?;
                validate_members(&cmd.members)?;
                Ok(vec![Self::Event::SweepTemplateCreated(
                    SweepTemplateCreated {
                        sweep_template_id: cmd.sweep_template_id,
                        name: cmd.name,
                        members: cmd.members,
                    },
                )])
            }

            Self::Command::UpdateSweepTemplate(cmd) => {
                let s = state.ok_or(SweepTemplateError::NotFound)?;
                if s.deleted {
                    return Err(SweepTemplateError::AlreadyDeleted);
                }
                validate_name(&cmd.name)?;
                validate_members(&cmd.members)?;
                if s.name == cmd.name && s.members == cmd.members {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::SweepTemplateUpdated(
                    SweepTemplateUpdated {
                        sweep_template_id: s.sweep_template_id,
                        name: cmd.name,
                        members: cmd.members,
                    },
                )])
            }

            Self::Command::DeleteSweepTemplate(cmd) => {
                let s = state.ok_or(SweepTemplateError::NotFound)?;
                if s.deleted {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::SweepTemplateDeleted(
                    SweepTemplateDeleted {
                        sweep_template_id: cmd.sweep_template_id,
                    },
                )])
            }

            Self::Command::SetDefaultSweepTemplate(cmd) => {
                let s = state.ok_or(SweepTemplateError::NotFound)?;
                if s.deleted {
                    return Err(SweepTemplateError::AlreadyDeleted);
                }
                Ok(vec![Self::Event::SweepTemplateDefaultSet(
                    SweepTemplateDefaultSet {
                        sweep_template_id: cmd.sweep_template_id,
                    },
                )])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, Self::Event::SweepTemplateCreated(e)) => {
                    state = Some(Self::from_created(e));
                }
                (Some(_), Self::Event::SweepTemplateCreated(_)) => return None,
                (None, _) => return None,
                (Some(s), event) => s.apply(event),
            }
        }
        state
    }
}

fn validate_name(name: &str) -> Result<(), SweepTemplateError> {
    if name.trim().is_empty() {
        return Err(SweepTemplateError::ValidationError(
            "sweep template name cannot be empty".into(),
        ));
    }
    Ok(())
}

fn validate_members(members: &[Uuid]) -> Result<(), SweepTemplateError> {
    if members.is_empty() {
        return Err(SweepTemplateError::ValidationError(
            "Sweep template must include at least one chunking configuration".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::commands::{
        CreateSweepTemplate, DeleteSweepTemplate, SetDefaultSweepTemplate, UpdateSweepTemplate,
    };
    use super::*;

    fn make_create_cmd(id: Uuid) -> SweepTemplateCommand {
        SweepTemplateCommand::CreateSweepTemplate(CreateSweepTemplate {
            sweep_template_id: id,
            name: "default".into(),
            members: vec![Uuid::new_v4()],
        })
    }

    fn make_created_event(id: Uuid) -> SweepTemplateEvent {
        SweepTemplateEvent::SweepTemplateCreated(SweepTemplateCreated {
            sweep_template_id: id,
            name: "default".into(),
            members: vec![Uuid::new_v4()],
        })
    }

    #[test]
    fn create_emits_created_event() {
        let id = Uuid::new_v4();
        let events = SweepTemplate::handle_command(None, make_create_cmd(id)).unwrap();
        assert_eq!(events.len(), 1);
        let state = SweepTemplate::from_events(&events).unwrap();
        assert_eq!(state.sweep_template_id, id);
        assert!(!state.deleted);
    }

    #[test]
    fn create_on_existing_fails() {
        let id = Uuid::new_v4();
        let state = SweepTemplate::from_events(&[make_created_event(id)]).unwrap();
        let err = SweepTemplate::handle_command(Some(&state), make_create_cmd(id)).unwrap_err();
        assert!(matches!(err, SweepTemplateError::AlreadyExists));
    }

    #[test]
    fn update_with_same_values_is_noop() {
        let id = Uuid::new_v4();
        let member = Uuid::new_v4();
        let events = vec![SweepTemplateEvent::SweepTemplateCreated(
            SweepTemplateCreated {
                sweep_template_id: id,
                name: "default".into(),
                members: vec![member],
            },
        )];
        let state = SweepTemplate::from_events(&events).unwrap();
        let out = SweepTemplate::handle_command(
            Some(&state),
            SweepTemplateCommand::UpdateSweepTemplate(UpdateSweepTemplate {
                sweep_template_id: id,
                name: "default".into(),
                members: vec![member],
            }),
        )
        .unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn update_with_empty_name_fails() {
        let id = Uuid::new_v4();
        let state = SweepTemplate::from_events(&[make_created_event(id)]).unwrap();
        let err = SweepTemplate::handle_command(
            Some(&state),
            SweepTemplateCommand::UpdateSweepTemplate(UpdateSweepTemplate {
                sweep_template_id: id,
                name: "  ".into(),
                members: vec![Uuid::new_v4()],
            }),
        )
        .unwrap_err();
        assert!(matches!(err, SweepTemplateError::ValidationError(_)));
    }

    #[test]
    fn delete_is_idempotent() {
        let id = Uuid::new_v4();
        let mut events = vec![make_created_event(id)];
        events.push(SweepTemplateEvent::SweepTemplateDeleted(
            SweepTemplateDeleted {
                sweep_template_id: id,
            },
        ));
        let state = SweepTemplate::from_events(&events).unwrap();
        let out = SweepTemplate::handle_command(
            Some(&state),
            SweepTemplateCommand::DeleteSweepTemplate(DeleteSweepTemplate {
                sweep_template_id: id,
            }),
        )
        .unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn set_default_emits_event() {
        let id = Uuid::new_v4();
        let state = SweepTemplate::from_events(&[make_created_event(id)]).unwrap();
        let out = SweepTemplate::handle_command(
            Some(&state),
            SweepTemplateCommand::SetDefaultSweepTemplate(SetDefaultSweepTemplate {
                sweep_template_id: id,
            }),
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert!(matches!(
            out[0],
            SweepTemplateEvent::SweepTemplateDefaultSet(_)
        ));
    }

    #[test]
    fn from_events_requires_created_first() {
        let result = SweepTemplate::from_events(&[SweepTemplateEvent::SweepTemplateDeleted(
            SweepTemplateDeleted {
                sweep_template_id: Uuid::new_v4(),
            },
        )]);
        assert!(result.is_none());
    }
}
