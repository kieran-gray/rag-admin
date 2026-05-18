use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::config::ConnectorConfig;

pub struct RegisterConnector {
    pub connector_id: Uuid,
    pub name: String,
    pub config: ConnectorConfig,
    pub occurred_at: Timestamp,
}

pub struct RenameConnector {
    pub connector_id: Uuid,
    pub name: String,
    pub occurred_at: Timestamp,
}

pub struct UpdateConnectorConfig {
    pub connector_id: Uuid,
    pub config: ConnectorConfig,
    pub occurred_at: Timestamp,
}

pub struct UnregisterConnector {
    pub connector_id: Uuid,
    pub occurred_at: Timestamp,
}

pub enum ConnectorCommand {
    RegisterConnector(RegisterConnector),
    RenameConnector(RenameConnector),
    UpdateConnectorConfig(UpdateConnectorConfig),
    UnregisterConnector(UnregisterConnector),
}

impl ConnectorCommand {
    pub fn connector_id(&self) -> Uuid {
        match self {
            ConnectorCommand::RegisterConnector(cmd) => cmd.connector_id,
            ConnectorCommand::RenameConnector(cmd) => cmd.connector_id,
            ConnectorCommand::UpdateConnectorConfig(cmd) => cmd.connector_id,
            ConnectorCommand::UnregisterConnector(cmd) => cmd.connector_id,
        }
    }
}
