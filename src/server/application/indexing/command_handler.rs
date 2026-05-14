use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::indexing::{aggregate::Indexing, commands::IndexingCommand};
use crate::server::event_sourcing::CommandProcessor;

pub struct IndexingCommandHandler {
    processor: Arc<CommandProcessor<Indexing>>,
}

impl IndexingCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<Indexing>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle(&self, command: IndexingCommand) -> Result<(), AppError> {
        let stream_id = match &command {
            IndexingCommand::RequestIngest(cmd) => {
                Indexing::compute_id(cmd.document_id, cmd.pipeline_configuration_id)
            }
            _ => panic!(
                "stage-completion commands must be dispatched via handle_for(aggregate_id, command)"
            ),
        };
        self.processor.handle(stream_id, command).await?;
        Ok(())
    }

    pub async fn handle_for(
        &self,
        aggregate_id: Uuid,
        command: IndexingCommand,
    ) -> Result<(), AppError> {
        self.processor.handle(aggregate_id, command).await?;
        Ok(())
    }
}
