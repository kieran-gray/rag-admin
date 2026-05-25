use crate::server::application::rerank::ports::Reranker;
use crate::server::application::Registry;
use crate::shared::reference_data::AiProviderKind;

pub type RerankerRegistry = Registry<AiProviderKind, dyn Reranker>;
