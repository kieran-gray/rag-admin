use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::AppError;

use super::ports::{ContentType, DocumentNormalizer, NormalizedDocument, RawDocument};

pub struct DocumentNormalizerRegistry {
    impls: HashMap<ContentType, Arc<dyn DocumentNormalizer>>,
}

impl DocumentNormalizerRegistry {
    pub fn new() -> Self {
        Self {
            impls: HashMap::new(),
        }
    }

    pub fn register(&mut self, normalizer: Arc<dyn DocumentNormalizer>) {
        self.impls.insert(normalizer.content_type(), normalizer);
    }

    pub async fn normalize(&self, raw: RawDocument) -> Result<NormalizedDocument, AppError> {
        let content_type = raw.content_type;
        let normalizer = self.impls.get(&content_type).ok_or_else(|| {
            AppError::Validation(format!(
                "no normalizer registered for content type {}",
                content_type.as_str()
            ))
        })?;
        normalizer.normalize(raw).await
    }
}

impl Default for DocumentNormalizerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
