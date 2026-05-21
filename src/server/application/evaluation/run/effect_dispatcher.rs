use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::process_manager::{EffectError, EffectExecutor};

use super::effect_executor::{CompleteRunEffectExecutor, ExecuteVariantEffectExecutor};
use super::optimize_effect_executor::OptimizeRunEffectExecutor;
use crate::server::domain::evaluation::run::effects::EvaluationRunEffect;

pub struct EvaluationRunEffectDispatcher {
    execute_variant: Arc<ExecuteVariantEffectExecutor>,
    complete_run: Arc<CompleteRunEffectExecutor>,
    optimize: Arc<OptimizeRunEffectExecutor>,
}

impl EvaluationRunEffectDispatcher {
    pub fn new(
        execute_variant: Arc<ExecuteVariantEffectExecutor>,
        complete_run: Arc<CompleteRunEffectExecutor>,
        optimize: Arc<OptimizeRunEffectExecutor>,
    ) -> Arc<Self> {
        Arc::new(Self {
            execute_variant,
            complete_run,
            optimize,
        })
    }
}

#[async_trait]
impl EffectExecutor<EvaluationRunEffect> for EvaluationRunEffectDispatcher {
    async fn execute(&self, effect: &EvaluationRunEffect) -> Result<(), EffectError> {
        let result = match effect {
            EvaluationRunEffect::ExecuteVariant(e) => self.execute_variant.run(e).await,
            EvaluationRunEffect::CompleteRun(e) => self.complete_run.run(e).await,
            EvaluationRunEffect::OptimizeRun(e) => self.optimize.run(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}
