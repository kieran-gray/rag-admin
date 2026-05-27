use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use crate::server::domain::comprehension::map::events::DocumentMapEvent;

pub struct EvaluationDatasetMapBridgeProjector;

impl EvaluationDatasetMapBridgeProjector {
    pub const NAME: &'static str = "evaluation_dataset_map_bridge_projector";

    pub fn new() -> Self {
        Self
    }
}

impl Default for EvaluationDatasetMapBridgeProjector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Projector<DocumentMapEvent> for EvaluationDatasetMapBridgeProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        _events: &[EventEnvelope<DocumentMapEvent>],
    ) -> Result<(), ProjectionError> {
        Ok(())
    }
}
