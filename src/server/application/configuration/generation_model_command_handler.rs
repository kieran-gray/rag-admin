use std::sync::Arc;

use crate::contracts::GenerationModelCommandDto;
use crate::event_sourcing::CommandProcessor;
use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::{
    AddGenerationModel, GenerationModelCatalog, GenerationModelCatalogCommand,
    RemoveGenerationModel, UpdateGenerationModel,
};

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
        self.handle(from_dto(command)).await
    }
}

fn from_dto(dto: GenerationModelCommandDto) -> GenerationModelCatalogCommand {
    match dto {
        GenerationModelCommandDto::AddGenerationModel(d) => {
            GenerationModelCatalogCommand::AddGenerationModel(AddGenerationModel {
                kind: d.kind,
                model: d.model,
            })
        }
        GenerationModelCommandDto::UpdateGenerationModel(d) => {
            GenerationModelCatalogCommand::UpdateGenerationModel(UpdateGenerationModel {
                model_id: d.model_id,
                kind: d.kind,
                model: d.model,
            })
        }
        GenerationModelCommandDto::RemoveGenerationModel(d) => {
            GenerationModelCatalogCommand::RemoveGenerationModel(RemoveGenerationModel {
                model_id: d.model_id,
            })
        }
    }
}
