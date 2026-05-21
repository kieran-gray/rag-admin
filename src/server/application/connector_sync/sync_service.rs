use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::connector::{ConnectorQueryService, ConnectorRegistry};
use crate::server::application::connector_sync::command_handler::ConnectorSyncCommandHandler;
use crate::server::application::ports::IdGenerator;
use crate::server::application::AppError;
use crate::server::domain::connector_sync::DiscoveredItemRecord;

pub struct ConnectorSyncService {
    command_handler: Arc<ConnectorSyncCommandHandler>,
    connector_registry: Arc<ConnectorRegistry>,
    connector_query_service: Arc<ConnectorQueryService>,
    id_generator: Arc<dyn IdGenerator>,
}

impl ConnectorSyncService {
    pub fn new(
        command_handler: Arc<ConnectorSyncCommandHandler>,
        connector_registry: Arc<ConnectorRegistry>,
        connector_query_service: Arc<ConnectorQueryService>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            command_handler,
            connector_registry,
            connector_query_service,
            id_generator,
        })
    }

    pub async fn run_sync(&self, connector_id: Uuid) -> Result<Uuid, AppError> {
        let connector = self
            .connector_query_service
            .get_config(connector_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("connector {connector_id} not found")))?;
        let implementation = self.connector_registry.get(connector.kind())?;

        let sync_id = self.id_generator.new_uuid();
        self.command_handler.start(sync_id, connector_id).await?;

        match implementation.list(&connector).await {
            Ok(items) => {
                let records: Vec<DiscoveredItemRecord> = items
                    .into_iter()
                    .map(|i| DiscoveredItemRecord {
                        source_ref_key: i.source_ref.natural_key(),
                        title: i.title,
                    })
                    .collect();
                if !records.is_empty() {
                    self.command_handler.record_items(sync_id, records).await?;
                }
                self.command_handler.complete(sync_id).await?;
                Ok(sync_id)
            }
            Err(e) => {
                self.command_handler.fail(sync_id, e.to_string()).await?;
                Err(e)
            }
        }
    }
}
