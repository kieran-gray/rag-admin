use std::sync::Arc;

use crate::server::application::indexing::ports::VectorIndex;
use crate::server::application::indexing::VectorIndexProviderRegistry;
use crate::server::application::source_document::ports::VectorIndexProvider;
use crate::server::infrastructure::shared::clients::CloudflareApi;
use crate::shared::reference_data::VectorStoreKind;

use super::named_vectorize_index::NamedVectorizeIndex;

pub struct CloudflareVectorIndexProvider {
    api: Arc<CloudflareApi>,
}

impl CloudflareVectorIndexProvider {
    pub fn new(api: Arc<CloudflareApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

impl VectorIndexProvider for CloudflareVectorIndexProvider {
    fn build(&self, index_name: &str, _dimensions: u32) -> Arc<dyn VectorIndex> {
        NamedVectorizeIndex::new(Arc::clone(&self.api), index_name.to_owned())
    }
}

pub fn register(registry: &mut VectorIndexProviderRegistry, api: Arc<CloudflareApi>) {
    registry.register(
        VectorStoreKind::CloudflareVectorize,
        CloudflareVectorIndexProvider::new(api),
    );
}
