use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entry::CatalogEntry;
use super::error::CatalogError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogState<E> {
    pub catalog_id: Uuid,
    pub entries: Vec<E>,
}

impl<E: CatalogEntry> CatalogState<E> {
    pub fn empty(catalog_id: Uuid) -> Self {
        Self {
            catalog_id,
            entries: Vec::new(),
        }
    }

    pub fn find(&self, entry_id: Uuid) -> Result<&E, CatalogError> {
        self.entries
            .iter()
            .find(|e| e.id() == entry_id)
            .ok_or(CatalogError::NotFound)
    }

    pub fn ensure_unique(
        &self,
        natural_key: &str,
        excluding: Option<Uuid>,
    ) -> Result<(), CatalogError> {
        if self
            .entries
            .iter()
            .any(|e| e.natural_key() == natural_key && Some(e.id()) != excluding)
        {
            return Err(CatalogError::ValidationError(format!(
                "catalog entry with key '{natural_key}' already exists"
            )));
        }
        Ok(())
    }

    pub fn push(&mut self, entry: E) {
        self.entries.push(entry);
    }

    pub fn update(&mut self, entry: E) {
        if let Some(slot) = self.entries.iter_mut().find(|e| e.id() == entry.id()) {
            *slot = entry;
        }
    }

    pub fn remove(&mut self, entry_id: Uuid) {
        self.entries.retain(|e| e.id() != entry_id);
    }
}
