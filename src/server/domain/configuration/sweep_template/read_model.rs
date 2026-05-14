use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepTemplateReadModel {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
    pub is_default: bool,
}
