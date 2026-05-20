use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::ports::Clock;
use crate::server::application::AppError;
use crate::server::domain::connector_sync::{
    CompleteConnectorSync, ConnectorSync, ConnectorSyncCommand, DiscoveredItemRecord,
    FailConnectorSync, RecordDiscoveredItems, StartConnectorSync,
};
use event_sourcing::CommandProcessor;

pub struct ConnectorSyncCommandHandler {
    processor: Arc<CommandProcessor<ConnectorSync>>,
    clock: Arc<dyn Clock>,
}

impl ConnectorSyncCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<ConnectorSync>>,
        clock: Arc<dyn Clock>,
    ) -> Arc<Self> {
        Arc::new(Self { processor, clock })
    }

    pub async fn start(&self, sync_id: Uuid, connector_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            sync_id,
            ConnectorSyncCommand::StartConnectorSync(StartConnectorSync {
                sync_id,
                connector_id,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn record_items(
        &self,
        sync_id: Uuid,
        items: Vec<DiscoveredItemRecord>,
    ) -> Result<(), AppError> {
        self.dispatch(
            sync_id,
            ConnectorSyncCommand::RecordDiscoveredItems(RecordDiscoveredItems {
                sync_id,
                items,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn complete(&self, sync_id: Uuid) -> Result<(), AppError> {
        self.dispatch(
            sync_id,
            ConnectorSyncCommand::CompleteConnectorSync(CompleteConnectorSync {
                sync_id,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    pub async fn fail(&self, sync_id: Uuid, error: String) -> Result<(), AppError> {
        self.dispatch(
            sync_id,
            ConnectorSyncCommand::FailConnectorSync(FailConnectorSync {
                sync_id,
                error,
                occurred_at: self.clock.now(),
            }),
        )
        .await
    }

    async fn dispatch(&self, sync_id: Uuid, command: ConnectorSyncCommand) -> Result<(), AppError> {
        self.processor.handle(sync_id, command).await?;
        Ok(())
    }
}
