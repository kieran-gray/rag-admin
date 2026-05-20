use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::config::ConnectorConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorReadModel {
    pub connector_id: Uuid,
    pub name: String,
    pub config: ConnectorConfig,
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub default_pipeline_configuration_id: Option<Uuid>,
    pub default_chunking_configuration_id: Option<Uuid>,
}
