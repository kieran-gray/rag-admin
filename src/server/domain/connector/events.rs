use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;

use super::config::ConnectorConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorRegistered {
    pub connector_id: Uuid,
    pub name: String,
    pub config: ConnectorConfig,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorRenamed {
    pub name: String,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorConfigUpdated {
    pub config: ConnectorConfig,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorUnregistered {
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectorDefaultsSet {
    pub index_profile_id: Option<Uuid>,
    pub chunking_configuration_id: Option<Uuid>,
    pub retrieval_profile_id: Option<Uuid>,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorEvent {
    ConnectorRegistered(ConnectorRegistered),
    ConnectorRenamed(ConnectorRenamed),
    ConnectorConfigUpdated(ConnectorConfigUpdated),
    ConnectorUnregistered(ConnectorUnregistered),
    ConnectorDefaultsSet(ConnectorDefaultsSet),
}
