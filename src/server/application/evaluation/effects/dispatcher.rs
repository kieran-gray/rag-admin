use std::sync::Arc;

use async_trait::async_trait;

use crate::event_sourcing::process_manager::{EffectError, EffectExecutor};

use super::optimize_executor::OptimizeRunEffectExecutor;
use super::run_executor::EvaluationRunEffectExecutor;
use crate::server::domain::evaluation::run::effects::EvaluationRunEffect;

pub struct EvaluationRunEffectDispatcher {
    execute: Arc<EvaluationRunEffectExecutor>,
    optimize: Arc<OptimizeRunEffectExecutor>,
}

impl EvaluationRunEffectDispatcher {
    pub fn new(
        execute: Arc<EvaluationRunEffectExecutor>,
        optimize: Arc<OptimizeRunEffectExecutor>,
    ) -> Arc<Self> {
        Arc::new(Self { execute, optimize })
    }
}

#[async_trait]
impl EffectExecutor<EvaluationRunEffect> for EvaluationRunEffectDispatcher {
    async fn execute(&self, effect: &EvaluationRunEffect) -> Result<(), EffectError> {
        let result = match effect {
            EvaluationRunEffect::ExecuteRun(e) => self.execute.run(e).await,
            EvaluationRunEffect::OptimizeRun(e) => self.optimize.run(e).await,
        };
        result.map_err(|e| Box::new(e) as EffectError)
    }
}
