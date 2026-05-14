use std::sync::Arc;

use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::configuration::sweep_template::{SweepTemplate, SweepTemplateCommand};
use crate::server::event_sourcing::CommandProcessor;
use crate::shared::SweepTemplateCommandDto;

pub struct SweepTemplateCommandHandler {
    processor: Arc<CommandProcessor<SweepTemplate>>,
    id_generator: Arc<dyn IdGenerator>,
}

impl SweepTemplateCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<SweepTemplate>>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            id_generator,
        })
    }

    pub async fn handle(&self, command: SweepTemplateCommand) -> Result<(), AppError> {
        let stream_id = command.sweep_template_id();
        self.processor.handle(stream_id, command).await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: SweepTemplateCommandDto) -> Result<(), AppError> {
        let cmd = SweepTemplateCommand::from_dto(command, || self.id_generator.new_uuid());
        self.handle(cmd).await
    }
}
