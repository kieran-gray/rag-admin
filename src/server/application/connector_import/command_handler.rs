use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::connector_import::{
    ConnectorImport, ConnectorImportCommand, RecordConnectorImport,
};
use crate::server::domain::shared::Timestamp;
use event_sourcing::CommandProcessor;

pub struct ConnectorImportCommandHandler {
    processor: Arc<CommandProcessor<ConnectorImport>>,
}

impl ConnectorImportCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<ConnectorImport>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn record(
        &self,
        connector_id: Uuid,
        document_id: Uuid,
        source_ref_key: String,
        sync_id: Option<Uuid>,
        occurred_at: Timestamp,
    ) -> Result<(), AppError> {
        let aggregate_id = ConnectorImport::compute_id(connector_id, document_id);
        self.processor
            .handle(
                aggregate_id,
                ConnectorImportCommand::RecordConnectorImport(RecordConnectorImport {
                    connector_id,
                    document_id,
                    source_ref_key,
                    sync_id,
                    occurred_at,
                }),
            )
            .await?;
        Ok(())
    }
}
