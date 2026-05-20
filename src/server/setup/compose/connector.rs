use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Notify;

use crate::server::application::connector::{
    ConnectorCommandHandler, ConnectorImpl, ConnectorQueryService, ConnectorRegistry,
};
use crate::server::application::connector_import::{
    ConnectorImportCommandHandler, ConnectorImportQueryService,
};
use crate::server::application::connector_sync::{
    ConnectorSyncCommandHandler, ConnectorSyncQueryService, ConnectorSyncService,
};
use crate::server::application::ports::{Clock, HttpClient, IdGenerator};
use crate::server::domain::connector::{Connector, ConnectorProjector};
use crate::server::domain::connector_import::{ConnectorImport, ConnectorImportProjector};
use crate::server::domain::connector_sync::{ConnectorSync, ConnectorSyncProjector};
use crate::server::infrastructure::connector::SitemapConnector;
use crate::server::infrastructure::shared::http::ReqwestHttpClient;
use crate::server::setup::compose::aggregates::{spawn_driver, AggregateWirings};
use crate::server::setup::compose::repositories::Repositories;
use crate::server::setup::exceptions::SetupError;
use event_sourcing::event_bus::EventBus;

pub struct ConnectorServices {
    pub connector_command_handler: Arc<ConnectorCommandHandler>,
    pub connector_query_service: Arc<ConnectorQueryService>,
    pub connector_import_command_handler: Arc<ConnectorImportCommandHandler>,
    pub connector_import_query_service: Arc<ConnectorImportQueryService>,
    pub connector_sync_command_handler: Arc<ConnectorSyncCommandHandler>,
    pub connector_sync_query_service: Arc<ConnectorSyncQueryService>,
    pub connector_sync_service: Arc<ConnectorSyncService>,
    pub connector_registry: Arc<ConnectorRegistry>,
}

pub struct ConnectorDeps<'a> {
    pub clock: Arc<dyn Clock>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub http: Arc<ReqwestHttpClient>,
    pub repos: &'a Repositories,
    pub wirings: &'a AggregateWirings,
    pub event_bus: Arc<EventBus>,
    pub wakeups: &'a mut HashMap<String, Arc<Notify>>,
}

impl ConnectorServices {
    pub fn build(deps: ConnectorDeps<'_>) -> Result<Self, SetupError> {
        let ConnectorDeps {
            clock,
            id_generator,
            http,
            repos,
            wirings,
            event_bus,
            wakeups,
        } = deps;

        let connector_query_service = ConnectorQueryService::new(Arc::clone(&repos.connector));
        let connector_command_handler = ConnectorCommandHandler::new(
            Arc::clone(&wirings.connector.command_processor),
            Arc::clone(&connector_query_service),
            Arc::clone(&clock),
            Arc::clone(&id_generator),
        );
        let connector_import_command_handler = ConnectorImportCommandHandler::new(
            Arc::clone(&wirings.connector_import.command_processor),
            Arc::clone(&clock),
        );
        let connector_import_query_service =
            ConnectorImportQueryService::new(Arc::clone(&repos.connector_import));
        let connector_sync_command_handler = ConnectorSyncCommandHandler::new(
            Arc::clone(&wirings.connector_sync.command_processor),
            Arc::clone(&clock),
        );
        let connector_sync_query_service = ConnectorSyncQueryService::new(
            Arc::clone(&repos.connector_sync),
            Arc::clone(&repos.connector_discovered_item),
            Arc::clone(&repos.connector_import),
            Arc::clone(&repos.source_document),
            Arc::clone(&repos.indexing),
        );

        let http_port: Arc<dyn HttpClient> = Arc::clone(&http) as Arc<dyn HttpClient>;
        let sitemap_connector: Arc<dyn ConnectorImpl> = SitemapConnector::new(http_port);
        let mut connector_registry = ConnectorRegistry::new();
        connector_registry.register(sitemap_connector);
        let connector_registry = Arc::new(connector_registry);

        let connector_sync_service = ConnectorSyncService::new(
            Arc::clone(&connector_sync_command_handler),
            Arc::clone(&connector_sync_query_service),
            Arc::clone(&connector_registry),
            Arc::clone(&connector_query_service),
            Arc::clone(&connector_import_command_handler),
            id_generator,
        );

        spawn_drivers(repos, wirings, &event_bus, wakeups);

        Ok(Self {
            connector_command_handler,
            connector_query_service,
            connector_import_command_handler,
            connector_import_query_service,
            connector_sync_command_handler,
            connector_sync_query_service,
            connector_sync_service,
            connector_registry,
        })
    }
}

fn spawn_drivers(
    repos: &Repositories,
    wirings: &AggregateWirings,
    event_bus: &Arc<EventBus>,
    wakeups: &mut HashMap<String, Arc<Notify>>,
) {
    spawn_driver::<Connector, ()>(
        Arc::clone(&wirings.connector.event_store),
        vec![Arc::new(ConnectorProjector::new(Arc::clone(
            &repos.connector,
        )))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<ConnectorImport, ()>(
        Arc::clone(&wirings.connector_import.event_store),
        vec![Arc::new(ConnectorImportProjector::new(Arc::clone(
            &repos.connector_import,
        )))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
    spawn_driver::<ConnectorSync, ()>(
        Arc::clone(&wirings.connector_sync.event_store),
        vec![Arc::new(ConnectorSyncProjector::new(
            Arc::clone(&repos.connector_sync),
            Arc::clone(&repos.connector_discovered_item),
        ))],
        None,
        Arc::clone(&repos.checkpoint),
        Arc::clone(event_bus),
        wakeups,
    );
}
