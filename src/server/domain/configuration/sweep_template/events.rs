use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepTemplateCreated {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepTemplateUpdated {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepTemplateDeleted {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepTemplateDefaultSet {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum SweepTemplateEvent {
    SweepTemplateCreated(SweepTemplateCreated),
    SweepTemplateUpdated(SweepTemplateUpdated),
    SweepTemplateDeleted(SweepTemplateDeleted),
    SweepTemplateDefaultSet(SweepTemplateDefaultSet),
}
