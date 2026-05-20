use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::configuration::catalog::{CatalogEntry, CatalogError};
use crate::shared::reference_data::AiProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationModel {
    pub generation_model_id: Uuid,
    pub kind: AiProviderKind,
    pub model: String,
}

impl CatalogEntry for GenerationModel {
    fn id(&self) -> Uuid {
        self.generation_model_id
    }

    fn natural_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.model)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        if self.model.trim().is_empty() {
            return Err(CatalogError::ValidationError(
                "generation model cannot be empty".into(),
            ));
        }
        if !self.kind.model_id_well_formed(&self.model) {
            return Err(CatalogError::ValidationError(format!(
                "model id '{}' is not well-formed for provider kind {}",
                self.model,
                self.kind.as_str()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(kind: AiProviderKind, name: &str) -> GenerationModel {
        GenerationModel {
            generation_model_id: Uuid::new_v4(),
            kind,
            model: name.into(),
        }
    }

    #[test]
    fn id_returns_inner_uuid() {
        let m = model(AiProviderKind::Ollama, "llama3");
        assert_eq!(m.id(), m.generation_model_id);
    }

    #[test]
    fn natural_key_combines_kind_and_model() {
        let m = model(AiProviderKind::Ollama, "llama3");
        assert_eq!(m.natural_key(), "ollama:llama3");
    }

    #[test]
    fn validate_accepts_cloudflare_id_with_at_cf_prefix() {
        assert!(model(AiProviderKind::Cloudflare, "@cf/meta/llama-3")
            .validate()
            .is_ok());
    }

    #[test]
    fn validate_rejects_cloudflare_id_without_at_cf_prefix() {
        let err = model(AiProviderKind::Cloudflare, "meta/llama-3")
            .validate()
            .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn validate_rejects_ollama_id_with_whitespace() {
        let err = model(AiProviderKind::Ollama, "llama 3")
            .validate()
            .unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn validate_rejects_empty_model_name() {
        let err = model(AiProviderKind::Ollama, "  ").validate().unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn validate_accepts_ollama_with_simple_name() {
        assert!(model(AiProviderKind::Ollama, "llama3").validate().is_ok());
    }
}
