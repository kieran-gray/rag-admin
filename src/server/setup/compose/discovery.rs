use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::discovery::{
    DiscoveryQueryService, IndexDiscoveryQueryService, ModelDiscoveryPort, VectorIndexDiscoveryPort,
};
use crate::server::infrastructure::discovery::{
    CloudflareDiscoveryAdapter, CloudflareVectorizeDiscoveryAdapter, OllamaDiscoveryAdapter,
};
use crate::server::infrastructure::shared::clients::{CloudflareApi, OllamaApi};
use crate::server::setup::exceptions::SetupError;
use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};

pub struct DiscoveryServices {
    pub discovery_query_service: Arc<DiscoveryQueryService>,
    pub index_discovery_query_service: Arc<IndexDiscoveryQueryService>,
}

pub struct DiscoveryDeps {
    pub cf_api: Arc<CloudflareApi>,
    pub ollama_api: Arc<OllamaApi>,
}

impl DiscoveryServices {
    pub fn build(deps: DiscoveryDeps) -> Result<Self, SetupError> {
        let mut adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>> = HashMap::new();
        adapters.insert(
            AiProviderKind::Cloudflare,
            CloudflareDiscoveryAdapter::new(deps.cf_api.clone()),
        );
        adapters.insert(
            AiProviderKind::Ollama,
            OllamaDiscoveryAdapter::new(deps.ollama_api),
        );

        let mut index_adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>> =
            HashMap::new();
        index_adapters.insert(
            VectorStoreKind::CloudflareVectorize,
            CloudflareVectorizeDiscoveryAdapter::new(deps.cf_api),
        );

        Ok(Self {
            discovery_query_service: DiscoveryQueryService::new(adapters),
            index_discovery_query_service: IndexDiscoveryQueryService::new(index_adapters),
        })
    }
}
