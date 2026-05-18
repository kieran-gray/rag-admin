pub mod bert;
pub mod darn;
pub mod llm;
pub mod section;

use std::sync::Arc;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::llm::ports::GenerationClient;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;

pub use bert::BertChunker;
pub use darn::DarnChunker;
pub use llm::LlmChunker;
pub use section::SectionChunker;

pub struct BuiltinChunkerDeps {
    pub generation_client: Arc<dyn GenerationClient>,
    pub generation_models: Arc<dyn GenerationModelRepository>,
}

pub fn register_builtin_chunkers(registry: &mut ChunkerRegistry, deps: BuiltinChunkerDeps) {
    registry.add(Arc::new(SectionChunker {}));
    registry.add(Arc::new(BertChunker {}));
    registry.add(Arc::new(DarnChunker {}));
    registry.add(Arc::new(LlmChunker::create(
        deps.generation_client,
        deps.generation_models,
    )));
}
