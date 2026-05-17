use std::sync::Arc;

use uuid::Uuid;

use crate::contracts::SweepTemplateCommandDto;
use crate::server::application::configuration::ConfigurationDefaultsCommandHandler;
use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::configuration::sweep_template::{
    CreateSweepTemplate, DeleteSweepTemplate, SweepTemplate, SweepTemplateCommand,
    UpdateSweepTemplate,
};
use crate::server::event_sourcing::CommandProcessor;

pub struct SweepTemplateCommandHandler {
    processor: Arc<CommandProcessor<SweepTemplate>>,
    defaults: Arc<ConfigurationDefaultsCommandHandler>,
    id_generator: Arc<dyn IdGenerator>,
}

impl SweepTemplateCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<SweepTemplate>>,
        defaults: Arc<ConfigurationDefaultsCommandHandler>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            defaults,
            id_generator,
        })
    }

    pub async fn handle(&self, command: SweepTemplateCommand) -> Result<(), AppError> {
        let stream_id = command.sweep_template_id();
        self.processor.handle(stream_id, command).await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: SweepTemplateCommandDto) -> Result<(), AppError> {
        match command {
            SweepTemplateCommandDto::SetDefaultSweepTemplate(d) => {
                self.defaults
                    .set_default_sweep_template(d.sweep_template_id)
                    .await
            }
            other => {
                let cmd = from_dto(other, || self.id_generator.new_uuid());
                self.handle(cmd).await
            }
        }
    }
}

fn from_dto(dto: SweepTemplateCommandDto, new_id: impl FnOnce() -> Uuid) -> SweepTemplateCommand {
    match dto {
        SweepTemplateCommandDto::CreateSweepTemplate(d) => {
            SweepTemplateCommand::CreateSweepTemplate(CreateSweepTemplate {
                sweep_template_id: new_id(),
                name: d.name,
                members: d.members,
            })
        }
        SweepTemplateCommandDto::UpdateSweepTemplate(d) => {
            SweepTemplateCommand::UpdateSweepTemplate(UpdateSweepTemplate {
                sweep_template_id: d.sweep_template_id,
                name: d.name,
                members: d.members,
            })
        }
        SweepTemplateCommandDto::DeleteSweepTemplate(d) => {
            SweepTemplateCommand::DeleteSweepTemplate(DeleteSweepTemplate {
                sweep_template_id: d.sweep_template_id,
            })
        }
        SweepTemplateCommandDto::SetDefaultSweepTemplate(_) => {
            unreachable!("SetDefault is routed through ConfigurationDefaultsCommandHandler")
        }
    }
}
