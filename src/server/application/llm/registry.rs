use crate::server::application::llm::ports::GenerationClient;
use crate::server::application::Registry;
use crate::shared::reference_data::AiProviderKind;

pub type GenerationClientRegistry = Registry<AiProviderKind, dyn GenerationClient>;
