use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::embedding_model::{
    AddEmbeddingModel, EmbeddingModelCatalog, EmbeddingModelCatalogCommand, RemoveEmbeddingModel,
    UpdateEmbeddingModel,
};
use crate::shared::contracts::EmbeddingModelCommandDto;
use event_sourcing::CommandProcessor;

pub struct EmbeddingModelCatalogCommandHandler {
    processor: Arc<CommandProcessor<EmbeddingModelCatalog>>,
}

impl EmbeddingModelCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<EmbeddingModelCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle_dto(&self, command: EmbeddingModelCommandDto) -> Result<(), AppError> {
        self.dispatch(command.into()).await
    }

    async fn dispatch(&self, command: EmbeddingModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(EmbeddingModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }
}

impl From<EmbeddingModelCommandDto> for EmbeddingModelCatalogCommand {
    fn from(value: EmbeddingModelCommandDto) -> Self {
        match value {
            EmbeddingModelCommandDto::AddEmbeddingModel(d) => {
                Self::AddEmbeddingModel(AddEmbeddingModel {
                    kind: d.kind,
                    model: d.model,
                    dimensions: d.dimensions,
                })
            }
            EmbeddingModelCommandDto::UpdateEmbeddingModel(d) => {
                Self::UpdateEmbeddingModel(UpdateEmbeddingModel {
                    model_id: d.model_id,
                    kind: d.kind,
                    model: d.model,
                    dimensions: d.dimensions,
                })
            }
            EmbeddingModelCommandDto::RemoveEmbeddingModel(d) => {
                Self::RemoveEmbeddingModel(RemoveEmbeddingModel {
                    model_id: d.model_id,
                })
            }
        }
    }
}
