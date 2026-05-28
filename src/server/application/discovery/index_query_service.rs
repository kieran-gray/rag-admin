use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::discovery::vector_index_discovery_port::VectorIndexDiscoveryPort;
use crate::shared::contracts::{
    DiscoveredIndexDto, IndexDiscoveryOutcomeDto, IndexDiscoveryResponseDto,
};
use crate::shared::reference_data::VectorStoreKind;

pub struct IndexDiscoveryQueryService {
    adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>>,
}

impl IndexDiscoveryQueryService {
    pub fn new(adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>>) -> Arc<Self> {
        Arc::new(Self { adapters })
    }

    pub async fn discover(&self, kind: VectorStoreKind) -> IndexDiscoveryResponseDto {
        let outcome = match self.adapters.get(&kind) {
            Some(adapter) => match adapter.discover().await {
                Ok(indexes) => sort_outcome(indexes),
                Err(err) => IndexDiscoveryOutcomeDto::Failure {
                    message: err.to_string(),
                },
            },
            None => IndexDiscoveryOutcomeDto::NotImplemented,
        };
        IndexDiscoveryResponseDto { kind, outcome }
    }
}

fn sort_outcome(mut indexes: Vec<DiscoveredIndexDto>) -> IndexDiscoveryOutcomeDto {
    indexes.sort_by_key(|i| i.name.to_ascii_lowercase());
    IndexDiscoveryOutcomeDto::Success { indexes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::application::AppError;
    use async_trait::async_trait;

    struct StubAdapter {
        indexes: Vec<DiscoveredIndexDto>,
        error: Option<AppError>,
    }

    #[async_trait]
    impl VectorIndexDiscoveryPort for StubAdapter {
        async fn discover(&self) -> Result<Vec<DiscoveredIndexDto>, AppError> {
            match &self.error {
                Some(AppError::Upstream(m)) => Err(AppError::Upstream(m.clone())),
                Some(other) => Err(AppError::Upstream(other.to_string())),
                None => Ok(self.indexes.clone()),
            }
        }
    }

    fn index(name: &str) -> DiscoveredIndexDto {
        DiscoveredIndexDto {
            name: name.into(),
            dimensions: 1024,
            metric: None,
            description: None,
        }
    }

    #[tokio::test]
    async fn dispatches_and_sorts() {
        let adapter = Arc::new(StubAdapter {
            indexes: vec![index("zeta"), index("alpha")],
            error: None,
        });
        let mut adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>> =
            HashMap::new();
        adapters.insert(VectorStoreKind::CloudflareVectorize, adapter);

        let service = IndexDiscoveryQueryService::new(adapters);
        let response = service.discover(VectorStoreKind::CloudflareVectorize).await;
        match response.outcome {
            IndexDiscoveryOutcomeDto::Success { indexes } => {
                assert_eq!(indexes[0].name, "alpha");
                assert_eq!(indexes[1].name, "zeta");
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_adapter_yields_not_implemented() {
        let adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>> = HashMap::new();
        let service = IndexDiscoveryQueryService::new(adapters);
        let response = service.discover(VectorStoreKind::Postgres).await;
        assert!(matches!(
            response.outcome,
            IndexDiscoveryOutcomeDto::NotImplemented
        ));
    }

    #[tokio::test]
    async fn surfaces_upstream_error_message() {
        let mut adapters: HashMap<VectorStoreKind, Arc<dyn VectorIndexDiscoveryPort>> =
            HashMap::new();
        adapters.insert(
            VectorStoreKind::CloudflareVectorize,
            Arc::new(StubAdapter {
                indexes: Vec::new(),
                error: Some(AppError::Upstream("cf vectorize: 401 — bad creds".into())),
            }),
        );
        let service = IndexDiscoveryQueryService::new(adapters);
        let response = service.discover(VectorStoreKind::CloudflareVectorize).await;
        match response.outcome {
            IndexDiscoveryOutcomeDto::Failure { message } => {
                assert!(message.contains("401"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }
}
