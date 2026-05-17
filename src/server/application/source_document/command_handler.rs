use std::sync::Arc;

use crate::event_sourcing::CommandProcessor;
use crate::server::application::AppError;
use crate::server::domain::source_document::{
    aggregate::SourceDocument, commands::SourceDocumentCommand,
};

pub struct SourceDocumentCommandHandler {
    processor: Arc<CommandProcessor<SourceDocument>>,
}

impl SourceDocumentCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<SourceDocument>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle(&self, command: SourceDocumentCommand) -> Result<(), AppError> {
        let stream_id = command.document_id();
        self.processor.handle(stream_id, command).await?;
        Ok(())
    }
}
