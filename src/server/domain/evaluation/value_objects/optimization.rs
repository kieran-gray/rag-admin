use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{
    OptimizationBudget as OptimizationBudgetDto, OptimizationConfig as OptimizationConfigDto,
    OptimizationScope as OptimizationScopeDto,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationBudget {
    Quick,
    #[default]
    Thorough,
    Exhaustive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationScope {
    Chunking,
    Retrieval,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OptimizationConfig {
    pub budget: OptimizationBudget,
    pub scope: OptimizationScope,
    pub judges_enabled: bool,
    pub seed: Option<u64>,
    pub fixed_chunking_configuration_id: Option<Uuid>,
}

impl From<OptimizationBudgetDto> for OptimizationBudget {
    fn from(b: OptimizationBudgetDto) -> Self {
        match b {
            OptimizationBudgetDto::Quick => Self::Quick,
            OptimizationBudgetDto::Thorough => Self::Thorough,
            OptimizationBudgetDto::Exhaustive => Self::Exhaustive,
        }
    }
}

impl From<OptimizationBudget> for OptimizationBudgetDto {
    fn from(b: OptimizationBudget) -> Self {
        match b {
            OptimizationBudget::Quick => Self::Quick,
            OptimizationBudget::Thorough => Self::Thorough,
            OptimizationBudget::Exhaustive => Self::Exhaustive,
        }
    }
}

impl From<OptimizationScopeDto> for OptimizationScope {
    fn from(s: OptimizationScopeDto) -> Self {
        match s {
            OptimizationScopeDto::Chunking => Self::Chunking,
            OptimizationScopeDto::Retrieval => Self::Retrieval,
            OptimizationScopeDto::Both => Self::Both,
        }
    }
}

impl From<OptimizationScope> for OptimizationScopeDto {
    fn from(s: OptimizationScope) -> Self {
        match s {
            OptimizationScope::Chunking => Self::Chunking,
            OptimizationScope::Retrieval => Self::Retrieval,
            OptimizationScope::Both => Self::Both,
        }
    }
}

impl From<OptimizationConfigDto> for OptimizationConfig {
    fn from(c: OptimizationConfigDto) -> Self {
        Self {
            budget: c.budget.into(),
            scope: c.scope.into(),
            judges_enabled: c.judges_enabled,
            seed: c.seed,
            fixed_chunking_configuration_id: c.fixed_chunking_configuration_id,
        }
    }
}

impl From<OptimizationConfig> for OptimizationConfigDto {
    fn from(c: OptimizationConfig) -> Self {
        Self {
            budget: c.budget.into(),
            scope: c.scope.into(),
            judges_enabled: c.judges_enabled,
            seed: c.seed,
            fixed_chunking_configuration_id: c.fixed_chunking_configuration_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimization_config_round_trips() {
        let original = OptimizationConfigDto {
            budget: OptimizationBudgetDto::Thorough,
            scope: OptimizationScopeDto::Retrieval,
            judges_enabled: true,
            seed: Some(42),
            fixed_chunking_configuration_id: Some(Uuid::nil()),
        };
        let domain: OptimizationConfig = original.clone().into();
        let wire = serde_json::to_string(&domain).unwrap();
        let decoded: OptimizationConfig = serde_json::from_str(&wire).unwrap();
        let restored: OptimizationConfigDto = decoded.into();
        assert_eq!(restored.budget, original.budget);
        assert_eq!(restored.scope, original.scope);
        assert_eq!(restored.judges_enabled, original.judges_enabled);
        assert_eq!(restored.seed, original.seed);
        assert_eq!(
            restored.fixed_chunking_configuration_id,
            original.fixed_chunking_configuration_id
        );
    }
}
