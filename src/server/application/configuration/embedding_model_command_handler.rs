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

    pub async fn handle(&self, command: EmbeddingModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(EmbeddingModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }

    pub async fn handle_dto(&self, command: EmbeddingModelCommandDto) -> Result<(), AppError> {
        self.handle(from_dto(command)).await
    }
}

fn from_dto(dto: EmbeddingModelCommandDto) -> EmbeddingModelCatalogCommand {
    match dto {
        EmbeddingModelCommandDto::AddEmbeddingModel(d) => {
            EmbeddingModelCatalogCommand::AddEmbeddingModel(AddEmbeddingModel {
                kind: d.kind,
                model: d.model,
                dimensions: d.dimensions,
            })
        }
        EmbeddingModelCommandDto::UpdateEmbeddingModel(d) => {
            EmbeddingModelCatalogCommand::UpdateEmbeddingModel(UpdateEmbeddingModel {
                model_id: d.model_id,
                kind: d.kind,
                model: d.model,
                dimensions: d.dimensions,
            })
        }
        EmbeddingModelCommandDto::RemoveEmbeddingModel(d) => {
            EmbeddingModelCatalogCommand::RemoveEmbeddingModel(RemoveEmbeddingModel {
                model_id: d.model_id,
            })
        }
    }
}
