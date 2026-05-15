use serde::{Deserialize, Serialize};

use crate::core::evaluation::EvaluationSettings;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SettingsDto {
    #[serde(default)]
    pub evaluation: EvaluationSettings,
}
