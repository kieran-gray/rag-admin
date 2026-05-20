use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::Clock;
use crate::server::application::AppError;
use crate::server::domain::connector_import::{
    ConnectorImport, ConnectorImportCommand, RecordConnectorImport,
};
use event_sourcing::CommandProcessor;

pub struct ConnectorImportCommandHandler {
    processor: Arc<CommandProcessor<ConnectorImport>>,
    clock: Arc<dyn Clock>,
}

impl ConnectorImportCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<ConnectorImport>>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self { processor, clock })
    }

    pub async fn record(
        &self,
        connector_id: Uuid,
        document_id: Uuid,
        source_ref_key: String,
        sync_id: Option<Uuid>,
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
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }
}
