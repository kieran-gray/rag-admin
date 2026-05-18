use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{ChunkingConfigurationDto, IndexingDto, PipelineConfigurationDto};

#[derive(Clone, Copy)]
pub struct ConfigSelection {
    pub pipeline_id: RwSignal<Option<Uuid>>,
    pub chunking_id: RwSignal<Option<Uuid>>,
}

impl ConfigSelection {
    pub fn new(
        pipelines: &[PipelineConfigurationDto],
        chunking: &[ChunkingConfigurationDto],
        indexings: &[IndexingDto],
    ) -> Self {
        let initial_pipeline = indexings
            .iter()
            .find(|i| !i.removed)
            .map(|i| i.pipeline_configuration_id)
            .or_else(|| {
                pipelines
                    .iter()
                    .find(|p| p.is_default)
                    .or_else(|| pipelines.first())
                    .map(|p| p.pipeline_configuration_id)
            });

        let initial_chunking = chunking
            .iter()
            .find(|c| c.is_default)
            .or_else(|| chunking.first())
            .map(|c| c.chunking_configuration_id);

        Self {
            pipeline_id: RwSignal::new(initial_pipeline),
            chunking_id: RwSignal::new(initial_chunking),
        }
    }
}
