use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::PipelineConfigurationDto;

#[derive(Clone, Copy)]
pub struct EvaluateSelection {
    pub dataset_id: RwSignal<Option<Uuid>>,
    pub run_id: RwSignal<Option<Uuid>>,
    pub pipeline_id: RwSignal<Option<Uuid>>,
}

impl EvaluateSelection {
    pub fn new(
        pipelines: &[PipelineConfigurationDto],
        initial_dataset: Option<Uuid>,
        initial_run: Option<Uuid>,
    ) -> Self {
        let initial_pipeline = pipelines
            .iter()
            .find(|p| p.is_default)
            .or_else(|| pipelines.first())
            .map(|p| p.pipeline_configuration_id);

        Self {
            dataset_id: RwSignal::new(initial_dataset),
            run_id: RwSignal::new(initial_run),
            pipeline_id: RwSignal::new(initial_pipeline),
        }
    }
}
