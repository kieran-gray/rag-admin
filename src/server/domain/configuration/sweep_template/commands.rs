use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSweepTemplate {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSweepTemplate {
    pub sweep_template_id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSweepTemplate {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDefaultSweepTemplate {
    pub sweep_template_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SweepTemplateCommand {
    CreateSweepTemplate(CreateSweepTemplate),
    UpdateSweepTemplate(UpdateSweepTemplate),
    DeleteSweepTemplate(DeleteSweepTemplate),
    SetDefaultSweepTemplate(SetDefaultSweepTemplate),
}

impl SweepTemplateCommand {
    pub fn sweep_template_id(&self) -> Uuid {
        match self {
            Self::CreateSweepTemplate(c) => c.sweep_template_id,
            Self::UpdateSweepTemplate(c) => c.sweep_template_id,
            Self::DeleteSweepTemplate(c) => c.sweep_template_id,
            Self::SetDefaultSweepTemplate(c) => c.sweep_template_id,
        }
    }
}
