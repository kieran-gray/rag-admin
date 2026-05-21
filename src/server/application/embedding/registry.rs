use crate::server::application::embedding::ports::Embedder;
use crate::server::application::Registry;
use crate::shared::reference_data::AiProviderKind;

pub type EmbedderRegistry = Registry<AiProviderKind, dyn Embedder>;
