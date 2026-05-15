use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::AppError;
use crate::server::event_sourcing::process_manager::EffectExecutor;

use super::optimize_executor::OptimizeRunEffectExecutor;
use super::run::EvaluationRunEffect;
use super::run_executor::EvaluationRunEffectExecutor;

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
    async fn execute(&self, effect: &EvaluationRunEffect) -> Result<(), AppError> {
        match effect {
            EvaluationRunEffect::ExecuteRun(e) => self.execute.run(e).await,
            EvaluationRunEffect::OptimizeRun(e) => self.optimize.run(e).await,
        }
    }
}
