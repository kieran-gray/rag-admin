use serde::{Deserialize, Serialize};

use super::evaluation::EvaluationSettings;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SettingsDto {
    #[serde(default)]
    pub evaluation: EvaluationSettings,
}
