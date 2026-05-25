use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::configuration::reranker_model::{
    AddRerankerModel, RemoveRerankerModel, RerankerModelCatalog, RerankerModelCatalogCommand,
    UpdateRerankerModel,
};
use crate::shared::contracts::RerankerModelCommandDto;
use event_sourcing::CommandProcessor;

pub struct RerankerModelCatalogCommandHandler {
    processor: Arc<CommandProcessor<RerankerModelCatalog>>,
}

impl RerankerModelCatalogCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<RerankerModelCatalog>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn handle_dto(&self, command: RerankerModelCommandDto) -> Result<(), AppError> {
        self.dispatch(command.into()).await
    }

    async fn dispatch(&self, command: RerankerModelCatalogCommand) -> Result<(), AppError> {
        self.processor
            .handle(RerankerModelCatalog::singleton_id(), command)
            .await?;
        Ok(())
    }
}

impl From<RerankerModelCommandDto> for RerankerModelCatalogCommand {
    fn from(value: RerankerModelCommandDto) -> Self {
        match value {
            RerankerModelCommandDto::AddRerankerModel(d) => Self::AddRerankerModel(AddRerankerModel {
                kind: d.kind,
                model: d.model,
            }),
            RerankerModelCommandDto::UpdateRerankerModel(d) => {
                Self::UpdateRerankerModel(UpdateRerankerModel {
                    model_id: d.model_id,
                    kind: d.kind,
                    model: d.model,
                })
            }
            RerankerModelCommandDto::RemoveRerankerModel(d) => {
                Self::RemoveRerankerModel(RemoveRerankerModel {
                    model_id: d.model_id,
                })
            }
        }
    }
}
