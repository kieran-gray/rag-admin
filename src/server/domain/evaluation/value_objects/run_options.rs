use serde::{Deserialize, Serialize};

use crate::shared::EvaluationRunOptions as EvaluationRunOptionsDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvaluationRunOptions {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub top_k: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub min_score_milli: u32,
}

impl Default for EvaluationRunOptions {
    fn default() -> Self {
        EvaluationRunOptionsDto::default().into()
    }
}

impl From<EvaluationRunOptionsDto> for EvaluationRunOptions {
    fn from(o: EvaluationRunOptionsDto) -> Self {
        Self {
            top_k: o.top_k,
            min_score_milli: o.min_score_milli,
        }
    }
}

impl From<EvaluationRunOptions> for EvaluationRunOptionsDto {
    fn from(o: EvaluationRunOptions) -> Self {
        Self {
            top_k: o.top_k,
            min_score_milli: o.min_score_milli,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_string_encoded_numbers() {
        let legacy = json!({"top_k": "5", "min_score_milli": "300"});
        let domain: EvaluationRunOptions = serde_json::from_value(legacy).unwrap();
        assert_eq!(domain.top_k, 5);
        assert_eq!(domain.min_score_milli, 300);
    }
}
