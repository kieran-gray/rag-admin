use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::indexing::{
    aggregate::Indexing,
    commands::{IndexingCommand, RequestIngest},
};
use event_sourcing::CommandProcessor;

pub struct IndexingCommandHandler {
    processor: Arc<CommandProcessor<Indexing>>,
}

impl IndexingCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<Indexing>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn request_ingest(&self, command: RequestIngest) -> Result<Uuid, AppError> {
        let stream_id =
            Indexing::compute_id(command.document_id, command.pipeline_configuration_id);
        self.processor
            .handle(stream_id, IndexingCommand::RequestIngest(command))
            .await?;
        Ok(stream_id)
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
