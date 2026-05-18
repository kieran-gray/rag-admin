use std::collections::HashMap;
use std::sync::Arc;

use crate::server::application::AppError;
use crate::server::domain::connector::ConnectorKind;

use super::ports::ConnectorImpl;

pub struct ConnectorRegistry {
    implementations: HashMap<ConnectorKind, Arc<dyn ConnectorImpl>>,
}

impl ConnectorRegistry {
    pub fn new() -> Self {
        Self {
            implementations: HashMap::new(),
        }
    }

    pub fn register(&mut self, implementation: Arc<dyn ConnectorImpl>) {
        self.implementations
            .insert(implementation.kind(), implementation);
    }

    pub fn get(&self, kind: ConnectorKind) -> Result<&Arc<dyn ConnectorImpl>, AppError> {
        self.implementations.get(&kind).ok_or_else(|| {
            AppError::Validation(format!(
                "no connector implementation registered for {}",
                kind.as_str()
            ))
        })
    }
}

impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
