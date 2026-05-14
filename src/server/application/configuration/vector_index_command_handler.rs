use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::vector_index::{
    VectorIndexCatalog, VectorIndexCatalogCommand,
};
use crate::server::event_sourcing::CommandProcessor;
use crate::shared::VectorIndexCommandDto;

pub struct VectorIndexCatalogCommandHandler {
    processor: Arc<CommandProcessor<VectorIndexCatalog>>,
}

impl VectorIndexCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<VectorIndexCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle(&self, command: VectorIndexCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(VectorIndexCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: VectorIndexCommandDto) -> Result<(), AppError> {
        self.handle(VectorIndexCatalogCommand::from_dto(command))
            .await
    }
}
