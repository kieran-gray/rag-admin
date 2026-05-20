use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::connector::{ConnectorQueryService, ConnectorRegistry};
use crate::server::application::connector_import::ConnectorImportCommandHandler;
use crate::server::application::connector_sync::command_handler::ConnectorSyncCommandHandler;
use crate::server::application::connector_sync::query_service::ConnectorSyncQueryService;
use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::connector_sync::DiscoveredItemRecord;

pub struct ConnectorSyncService {
    command_handler: Arc<ConnectorSyncCommandHandler>,
    query_service: Arc<ConnectorSyncQueryService>,
    connector_registry: Arc<ConnectorRegistry>,
    connector_query_service: Arc<ConnectorQueryService>,
    connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl ConnectorSyncService {
    pub fn new(
        command_handler: Arc<ConnectorSyncCommandHandler>,
        query_service: Arc<ConnectorSyncQueryService>,
        connector_registry: Arc<ConnectorRegistry>,
        connector_query_service: Arc<ConnectorQueryService>,
        connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            command_handler,
            query_service,
            connector_registry,
            connector_query_service,
            connector_import_command_handler,
            clock,
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
        self.command_handler
            .start(sync_id, connector_id, self.clock.now())
            .await?;

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
                    self.command_handler
                        .record_items(sync_id, records, self.clock.now())
                        .await?;
                }
                self.command_handler
                    .complete(sync_id, self.clock.now())
                    .await?;
                self.backfill_provenance(connector_id, sync_id).await?;
                Ok(sync_id)
            }
            Err(e) => {
                self.command_handler
                    .fail(sync_id, e.to_string(), self.clock.now())
                    .await?;
                Err(e)
            }
        }
    }

    async fn backfill_provenance(&self, connector_id: Uuid, sync_id: Uuid) -> Result<(), AppError> {
        let pairs = self
            .query_service
            .discovered_items_missing_import(connector_id)
            .await?;
        for (source_ref_key, document_id) in pairs {
            self.connector_import_command_handler
                .record(
                    connector_id,
                    document_id,
                    source_ref_key,
                    Some(sync_id),
                    self.clock.now(),
                )
                .await?;
        }
        Ok(())
    }
}
