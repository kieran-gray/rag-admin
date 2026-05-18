use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::vector_index::{
    AddVectorIndex, RemoveVectorIndex, UpdateVectorIndex, VectorIndexCatalog,
    VectorIndexCatalogCommand,
};
use crate::shared::contracts::VectorIndexCommandDto;
use event_sourcing::CommandProcessor;

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
        self.handle(from_dto(command)).await
    }
}

fn from_dto(dto: VectorIndexCommandDto) -> VectorIndexCatalogCommand {
    match dto {
        VectorIndexCommandDto::AddVectorIndex(d) => {
            VectorIndexCatalogCommand::AddVectorIndex(AddVectorIndex {
                kind: d.kind,
                name: d.name,
                dimensions: d.dimensions,
            })
        }
        VectorIndexCommandDto::UpdateVectorIndex(d) => {
            VectorIndexCatalogCommand::UpdateVectorIndex(UpdateVectorIndex {
                index_id: d.index_id,
                kind: d.kind,
                name: d.name,
                dimensions: d.dimensions,
            })
        }
        VectorIndexCommandDto::RemoveVectorIndex(d) => {
            VectorIndexCatalogCommand::RemoveVectorIndex(RemoveVectorIndex {
                index_id: d.index_id,
            })
        }
    }
}
