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

    pub async fn handle_dto(&self, command: VectorIndexCommandDto) -> Result<(), AppError> {
        self.dispatch(command.into()).await
    }

    async fn dispatch(&self, command: VectorIndexCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(VectorIndexCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }
}

impl From<VectorIndexCommandDto> for VectorIndexCatalogCommand {
    fn from(value: VectorIndexCommandDto) -> Self {
        match value {
            VectorIndexCommandDto::AddVectorIndex(d) => Self::AddVectorIndex(AddVectorIndex {
                kind: d.kind,
                name: d.name,
                dimensions: d.dimensions,
            }),
            VectorIndexCommandDto::UpdateVectorIndex(d) => {
                Self::UpdateVectorIndex(UpdateVectorIndex {
                    index_id: d.index_id,
                    kind: d.kind,
                    name: d.name,
                    dimensions: d.dimensions,
                })
            }
            VectorIndexCommandDto::RemoveVectorIndex(d) => {
                Self::RemoveVectorIndex(RemoveVectorIndex {
                    index_id: d.index_id,
                })
            }
        }
    }
}
