use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::Clock;
use crate::server::application::AppError;
use crate::server::domain::connector_import::{
    CompleteConnectorImport, ConnectorImport, ConnectorImportCommand, FailConnectorImport,
    RecordItemFailed, RecordItemImported, StartConnectorImport,
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

    pub async fn start(
        &self,
        import_id: Uuid,
        connector_id: Uuid,
        source_ref_keys: Vec<String>,
        index_after_import: bool,
    ) -> Result<(), AppError> {
        self.dispatch(
            import_id,
            ConnectorImportCommand::StartConnectorImport(StartConnectorImport {
                import_id,
                connector_id,
                source_ref_keys,
                index_after_import,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn record_imported(
        &self,
        import_id: Uuid,
        source_ref_key: String,
        document_id: Uuid,
        document_version: u32,
    ) -> Result<(), AppError> {
        self.dispatch(
            import_id,
            ConnectorImportCommand::RecordItemImported(RecordItemImported {
                import_id,
                source_ref_key,
                document_id,
                document_version,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn record_failed(
        &self,
        import_id: Uuid,
        source_ref_key: String,
        error: String,
    ) -> Result<(), AppError> {
        self.dispatch(
            import_id,
            ConnectorImportCommand::RecordItemFailed(RecordItemFailed {
                import_id,
                source_ref_key,
                error,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn complete(&self, import_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            import_id,
            ConnectorImportCommand::CompleteConnectorImport(CompleteConnectorImport {
                import_id,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn fail(&self, import_id: Uuid, error: String) -> Result<(), AppError> {
        self.dispatch(
            import_id,
            ConnectorImportCommand::FailConnectorImport(FailConnectorImport {
                import_id,
                error,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    async fn dispatch(
        &self,
        import_id: Uuid,
        command: ConnectorImportCommand,
    ) -> Result<(), AppError> {
        self.processor.handle(import_id, command).await?;
        Ok(())
    }
}
