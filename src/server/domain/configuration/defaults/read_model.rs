use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigurationDefaultsReadModel {
    pub chunking_configuration_id: Option<Uuid>,
    pub pipeline_configuration_id: Option<Uuid>,
    pub sweep_template_id: Option<Uuid>,
}
