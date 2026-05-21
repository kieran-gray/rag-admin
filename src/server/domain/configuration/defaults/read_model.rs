use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigurationDefaultsReadModel {
    pub chunking_configuration_id: Option<Uuid>,
    pub index_profile_id: Option<Uuid>,
    pub retrieval_profile_id: Option<Uuid>,
    pub sweep_template_id: Option<Uuid>,
}
