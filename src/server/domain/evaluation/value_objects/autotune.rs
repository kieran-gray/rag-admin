use serde::{Deserialize, Serialize};

use crate::shared::EvaluationAutotuneRequest as EvaluationAutotuneRequestDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationAutotuneRequest {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub tuning_fraction_milli: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub holdout_top_n: u32,
}

impl From<EvaluationAutotuneRequestDto> for EvaluationAutotuneRequest {
    fn from(r: EvaluationAutotuneRequestDto) -> Self {
        Self {
            tuning_fraction_milli: r.tuning_fraction_milli,
            holdout_top_n: r.holdout_top_n,
        }
    }
}

impl From<EvaluationAutotuneRequest> for EvaluationAutotuneRequestDto {
    fn from(r: EvaluationAutotuneRequest) -> Self {
        Self {
            tuning_fraction_milli: r.tuning_fraction_milli,
            holdout_top_n: r.holdout_top_n,
        }
    }
}
