use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConnectorImportEffect {
    pub import_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConnectorImportEffect {
    Run(RunConnectorImportEffect),
}
