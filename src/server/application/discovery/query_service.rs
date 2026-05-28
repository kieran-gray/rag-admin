use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::discovery::model_discovery_port::ModelDiscoveryPort;
use crate::shared::contracts::{DiscoveredModelDto, DiscoveryOutcomeDto, DiscoveryResponseDto};
use crate::shared::reference_data::{AiProviderKind, ModelCapability};

pub struct DiscoveryQueryService {
    adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>>,
}

impl DiscoveryQueryService {
    pub fn new(adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>>) -> Arc<Self> {
        Arc::new(Self { adapters })
    }

    pub async fn discover(
        &self,
        provider: AiProviderKind,
        capability: ModelCapability,
    ) -> DiscoveryResponseDto {
        let outcome = match self.adapters.get(&provider) {
            Some(adapter) => match adapter.discover(capability).await {
                Ok(models) => sort_outcome(models),
                Err(err) => DiscoveryOutcomeDto::Failure {
                    message: err.to_string(),
                },
            },
            None => DiscoveryOutcomeDto::NotImplemented,
        };
        DiscoveryResponseDto {
            provider,
            capability,
            outcome,
        }
    }
}

fn sort_outcome(mut models: Vec<DiscoveredModelDto>) -> DiscoveryOutcomeDto {
    models.sort_by(|a, b| {
        let a_label = a.display_name.as_deref().unwrap_or(&a.id);
        let b_label = b.display_name.as_deref().unwrap_or(&b.id);
        a_label
            .to_ascii_lowercase()
            .cmp(&b_label.to_ascii_lowercase())
    });
    DiscoveryOutcomeDto::Success { models }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::application::AppError;
    use async_trait::async_trait;

    struct StubAdapter {
        models: Vec<DiscoveredModelDto>,
        error: Option<AppError>,
    }

    impl StubAdapter {
        fn ok(models: Vec<DiscoveredModelDto>) -> Arc<Self> {
            Arc::new(Self {
                models,
                error: None,
            })
        }

        fn err(error: AppError) -> Arc<Self> {
            Arc::new(Self {
                models: Vec::new(),
                error: Some(error),
            })
        }
    }

    #[async_trait]
    impl ModelDiscoveryPort for StubAdapter {
        async fn discover(
            &self,
            _capability: ModelCapability,
        ) -> Result<Vec<DiscoveredModelDto>, AppError> {
            match &self.error {
                Some(AppError::Upstream(m)) => Err(AppError::Upstream(m.clone())),
                Some(AppError::Internal(m)) => Err(AppError::Internal(m.clone())),
                Some(AppError::NotFound(m)) => Err(AppError::NotFound(m.clone())),
                Some(AppError::Validation(m)) => Err(AppError::Validation(m.clone())),
                Some(AppError::Io(m)) => Err(AppError::Io(m.clone())),
                None => Ok(self.models.clone()),
            }
        }
    }

    fn dto(id: &str, display: Option<&str>) -> DiscoveredModelDto {
        DiscoveredModelDto {
            id: id.into(),
            display_name: display.map(String::from),
            description: None,
            dimensions: None,
            context_length: None,
            parameter_size: None,
            quantization: None,
            family: None,
            size_bytes: None,
            capability_hint: Some(ModelCapability::Generation),
        }
    }

    #[tokio::test]
    async fn dispatches_to_registered_adapter_and_sorts_models() {
        let adapter = StubAdapter::ok(vec![
            dto("zeta-model", Some("Zeta")),
            dto("alpha-model", Some("Alpha")),
        ]);
        let mut adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>> = HashMap::new();
        adapters.insert(AiProviderKind::Ollama, adapter);

        let service = DiscoveryQueryService::new(adapters);
        let response = service
            .discover(AiProviderKind::Ollama, ModelCapability::Generation)
            .await;
        match response.outcome {
            DiscoveryOutcomeDto::Success { models } => {
                assert_eq!(models[0].id, "alpha-model");
                assert_eq!(models[1].id, "zeta-model");
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_adapter_yields_not_implemented() {
        let adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>> = HashMap::new();
        let service = DiscoveryQueryService::new(adapters);
        let response = service
            .discover(AiProviderKind::LlamaServer, ModelCapability::Generation)
            .await;
        assert!(matches!(
            response.outcome,
            DiscoveryOutcomeDto::NotImplemented
        ));
    }

    #[tokio::test]
    async fn surfaces_upstream_error_message() {
        let mut adapters: HashMap<AiProviderKind, Arc<dyn ModelDiscoveryPort>> = HashMap::new();
        adapters.insert(
            AiProviderKind::Ollama,
            StubAdapter::err(AppError::Upstream("ollama tags: 500 — boom".into())),
        );
        let service = DiscoveryQueryService::new(adapters);
        let response = service
            .discover(AiProviderKind::Ollama, ModelCapability::Generation)
            .await;
        match response.outcome {
            DiscoveryOutcomeDto::Failure { message } => {
                assert!(message.contains("boom"));
            }
            other => panic!("expected failure, got {other:?}"),
        }
    }
}
