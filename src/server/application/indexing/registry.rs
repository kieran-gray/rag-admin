use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::application::Registry;
use crate::shared::reference_data::VectorStoreKind;

pub type VectorIndexProviderRegistry = Registry<VectorStoreKind, dyn VectorIndexProvider>;
