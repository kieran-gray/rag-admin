use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::connector_sync::{
    CompleteConnectorSync, ConnectorSync, ConnectorSyncCommand, DiscoveredItemRecord,
    FailConnectorSync, RecordDiscoveredItems, StartConnectorSync,
};
use crate::server::domain::shared::Timestamp;
use event_sourcing::CommandProcessor;

pub struct ConnectorSyncCommandHandler {
    processor: Arc<CommandProcessor<ConnectorSync>>,
}

impl ConnectorSyncCommandHandler {
    pub fn new(processor: Arc<CommandProcessor<ConnectorSync>>) -> Arc<Self> {
        Arc::new(Self { processor })
    }

    pub async fn start(
        &self,
        sync_id: Uuid,
        connector_id: Uuid,
        occurred_at: Timestamp,
    ) -> Result<(), AppError> {
        self.processor
            .handle(
                sync_id,
                ConnectorSyncCommand::StartConnectorSync(StartConnectorSync {
                    sync_id,
                    connector_id,
                    occurred_at,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn record_items(
        &self,
        sync_id: Uuid,
        items: Vec<DiscoveredItemRecord>,
        occurred_at: Timestamp,
    ) -> Result<(), AppError> {
        self.processor
            .handle(
                sync_id,
                ConnectorSyncCommand::RecordDiscoveredItems(RecordDiscoveredItems {
                    sync_id,
                    items,
                    occurred_at,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn complete(&self, sync_id: Uuid, occurred_at: Timestamp) -> Result<(), AppError> {
        self.processor
            .handle(
                sync_id,
                ConnectorSyncCommand::CompleteConnectorSync(CompleteConnectorSync {
                    sync_id,
                    occurred_at,
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn fail(
        &self,
        sync_id: Uuid,
        error: String,
        occurred_at: Timestamp,
    ) -> Result<(), AppError> {
        self.processor
            .handle(
                sync_id,
                ConnectorSyncCommand::FailConnectorSync(FailConnectorSync {
                    sync_id,
                    error,
                    occurred_at,
                }),
            )
            .await?;
        Ok(())
    }
}
