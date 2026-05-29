use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;

use super::command_handler::ConnectorImportCommandHandler;

pub struct ConnectorImportService {
    command_handler: Arc<ConnectorImportCommandHandler>,
    id_generator: Arc<dyn IdGenerator>,
}

impl ConnectorImportService {
    pub fn new(
        command_handler: Arc<ConnectorImportCommandHandler>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            command_handler,
            id_generator,
        })
    }

    pub async fn start(
        &self,
        connector_id: Uuid,
        source_ref_keys: Vec<String>,
        index_after_import: bool,
    ) -> Result<Uuid, AppError> {
        if source_ref_keys.is_empty() {
            return Err(AppError::Validation("no items selected for import".into()));
        }
        let import_id = self.id_generator.new_uuid();
        self.command_handler
            .start(import_id, connector_id, source_ref_keys, index_after_import)
            .await?;
        Ok(import_id)
    }
}
