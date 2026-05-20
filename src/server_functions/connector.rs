use leptos::prelude::*;

use crate::shared::contracts::{ConnectorCommandDto, ConnectorDto};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::connector::ConnectorServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = ListConnectors, prefix = "/api", endpoint = "list_connectors")]
pub async fn list_connectors() -> Result<Vec<ConnectorDto>, ServerFnError> {
    ctx::<Arc<ConnectorServices>>()?
        .connector_query_service
        .list()
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(
    name = ApplyConnectorCommand,
    prefix = "/api",
    endpoint = "apply_connector_command"
)]
pub async fn apply_connector_command(
    command: ConnectorCommandDto,
) -> Result<Option<ConnectorDto>, ServerFnError> {
    ctx::<Arc<ConnectorServices>>()?
        .connector_command_handler
        .handle_dto(command)
        .await
        .map_err(|e| map_app_error(&e))
}
