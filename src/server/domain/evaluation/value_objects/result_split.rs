use serde::{Deserialize, Serialize};

use crate::shared::EvaluationResultSplit as EvaluationResultSplitDto;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationResultSplit {
    #[default]
    Full,
    Tuning,
    Validation,
    Holdout,
}

impl EvaluationResultSplit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Tuning => "tuning",
            Self::Validation => "validation",
            Self::Holdout => "holdout",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "full" => Ok(Self::Full),
            "tuning" => Ok(Self::Tuning),
            "validation" => Ok(Self::Validation),
            "holdout" => Ok(Self::Holdout),
            other => Err(format!("unknown evaluation split '{other}'")),
        }
    }
}

impl From<EvaluationResultSplitDto> for EvaluationResultSplit {
    fn from(s: EvaluationResultSplitDto) -> Self {
        match s {
            EvaluationResultSplitDto::Full => Self::Full,
            EvaluationResultSplitDto::Tuning => Self::Tuning,
            EvaluationResultSplitDto::Validation => Self::Validation,
            EvaluationResultSplitDto::Holdout => Self::Holdout,
        }
    }
}

impl From<EvaluationResultSplit> for EvaluationResultSplitDto {
    fn from(s: EvaluationResultSplit) -> Self {
        match s {
            EvaluationResultSplit::Full => Self::Full,
            EvaluationResultSplit::Tuning => Self::Tuning,
            EvaluationResultSplit::Validation => Self::Validation,
            EvaluationResultSplit::Holdout => Self::Holdout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_serializes_snake_case() {
        let domain: EvaluationResultSplit = EvaluationResultSplitDto::Holdout.into();
        let wire = serde_json::to_value(&domain).unwrap();
        assert_eq!(wire, json!("holdout"));
    }
}
