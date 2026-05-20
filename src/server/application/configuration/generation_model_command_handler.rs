use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::{
    AddGenerationModel, GenerationModelCatalog, GenerationModelCatalogCommand,
    RemoveGenerationModel, UpdateGenerationModel,
};
use crate::shared::contracts::GenerationModelCommandDto;
use event_sourcing::CommandProcessor;

pub struct GenerationModelCatalogCommandHandler {
    processor: Arc<CommandProcessor<GenerationModelCatalog>>,
}

impl GenerationModelCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<GenerationModelCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle_dto(&self, command: GenerationModelCommandDto) -> Result<(), AppError> {
        self.dispatch(command.into()).await
    }

    async fn dispatch(&self, command: GenerationModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(GenerationModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }
}

impl From<GenerationModelCommandDto> for GenerationModelCatalogCommand {
    fn from(value: GenerationModelCommandDto) -> Self {
        match value {
            GenerationModelCommandDto::AddGenerationModel(d) => {
                Self::AddGenerationModel(AddGenerationModel {
                    kind: d.kind,
                    model: d.model,
                })
            }
            GenerationModelCommandDto::UpdateGenerationModel(d) => {
                Self::UpdateGenerationModel(UpdateGenerationModel {
                    model_id: d.model_id,
                    kind: d.kind,
                    model: d.model,
                })
            }
            GenerationModelCommandDto::RemoveGenerationModel(d) => {
                Self::RemoveGenerationModel(RemoveGenerationModel {
                    model_id: d.model_id,
                })
            }
        }
    }
}
