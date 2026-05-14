pub mod bert;
pub mod llm;
pub mod section;

use std::sync::Arc;

use crate::server::application::chunking::ChunkerRegistry;
use crate::server::application::ports::ChatClient;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;

pub use bert::BertChunker;
pub use llm::LlmChunker;
pub use section::SectionChunker;

pub struct BuiltinChunkerDeps {
    pub chat_client: Arc<dyn ChatClient>,
    pub generation_models: Arc<dyn GenerationModelRepository>,
}

pub fn register_builtin_chunkers(registry: &mut ChunkerRegistry, deps: BuiltinChunkerDeps) {
    registry.add(Arc::new(SectionChunker {}));
    registry.add(Arc::new(BertChunker {}));
    registry.add(Arc::new(LlmChunker::create(
        deps.chat_client,
        deps.generation_models,
    )));
}
