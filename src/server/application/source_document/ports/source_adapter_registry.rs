use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::source_document::document_type::DocumentType;

use super::source_adapter::{DocumentSummary, SourceAdapter};

pub struct SourceAdapterRegistry {
    adapters: HashMap<String, Arc<dyn SourceAdapter>>,
}

impl SourceAdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn SourceAdapter>) {
        let key = format!("{:?}", adapter.document_type());
        self.adapters.insert(key, adapter);
    }

    pub fn get(&self, document_type: &DocumentType) -> Option<&Arc<dyn SourceAdapter>> {
        let key = format!("{:?}", document_type);
        self.adapters.get(&key)
    }

    pub async fn list_all(&self) -> Result<Vec<(String, DocumentSummary)>, AppError> {
        let mut result = Vec::new();
        for (type_key, adapter) in &self.adapters {
            let summaries = adapter.list().await?;
            for summary in summaries {
                result.push((type_key.clone(), summary));
            }
        }
        Ok(result)
    }
}

impl Default for SourceAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
