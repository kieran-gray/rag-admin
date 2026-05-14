use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::{
    GenerationModelCatalog, GenerationModelCatalogCommand,
};
use crate::server::event_sourcing::CommandProcessor;
use crate::shared::GenerationModelCommandDto;

pub struct GenerationModelCatalogCommandHandler {
    processor: Arc<CommandProcessor<GenerationModelCatalog>>,
}

impl GenerationModelCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<GenerationModelCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle(&self, command: GenerationModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(GenerationModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: GenerationModelCommandDto) -> Result<(), AppError> {
        self.handle(GenerationModelCatalogCommand::from_dto(command))
            .await
    }
}
