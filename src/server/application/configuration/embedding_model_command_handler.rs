use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::{
    EmbeddingModelCatalog, EmbeddingModelCatalogCommand,
};
use crate::server::event_sourcing::CommandProcessor;
use crate::shared::EmbeddingModelCommandDto;

pub struct EmbeddingModelCatalogCommandHandler {
    processor: Arc<CommandProcessor<EmbeddingModelCatalog>>,
}

impl EmbeddingModelCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<EmbeddingModelCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle(&self, command: EmbeddingModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(EmbeddingModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: EmbeddingModelCommandDto) -> Result<(), AppError> {
        self.handle(EmbeddingModelCatalogCommand::from_dto(command))
            .await
    }
}
