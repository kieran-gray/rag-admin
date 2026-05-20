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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEntry {
        id: Uuid,
        key: String,
        valid: bool,
    }

    impl CatalogEntry for TestEntry {
        fn id(&self) -> Uuid {
            self.id
        }
        fn natural_key(&self) -> String {
            self.key.clone()
        }
        fn validate(&self) -> Result<(), CatalogError> {
            if self.valid {
                Ok(())
            } else {
                Err(CatalogError::ValidationError("invalid".into()))
            }
        }
    }

    fn entry(key: &str) -> TestEntry {
        TestEntry {
            id: Uuid::new_v4(),
            key: key.into(),
            valid: true,
        }
    }

    #[test]
    fn empty_starts_with_no_entries_and_remembers_catalog_id() {
        let cid = Uuid::new_v4();
        let s: CatalogState<TestEntry> = CatalogState::empty(cid);
        assert_eq!(s.catalog_id, cid);
        assert!(s.entries.is_empty());
    }

    #[test]
    fn find_returns_entry_by_id() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let e = entry("a");
        let id = e.id;
        s.push(e);
        let found = s.find(id).expect("present");
        assert_eq!(found.key, "a");
    }

    #[test]
    fn find_returns_not_found_for_missing_id() {
        let s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let err = s.find(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, CatalogError::NotFound));
    }

    #[test]
    fn ensure_unique_passes_when_no_collision() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        s.push(entry("a"));
        assert!(s.ensure_unique("b", None).is_ok());
    }

    #[test]
    fn ensure_unique_rejects_existing_key() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        s.push(entry("a"));
        let err = s.ensure_unique("a", None).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn ensure_unique_lets_excluded_entry_keep_its_key() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let e = entry("a");
        let id = e.id;
        s.push(e);
        assert!(s.ensure_unique("a", Some(id)).is_ok());
    }

    #[test]
    fn ensure_unique_still_rejects_when_different_entry_owns_key() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let owner = entry("a");
        let other = entry("b");
        let other_id = other.id;
        s.push(owner);
        s.push(other);
        let err = s.ensure_unique("a", Some(other_id)).unwrap_err();
        assert!(matches!(err, CatalogError::ValidationError(_)));
    }

    #[test]
    fn update_replaces_entry_by_id() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let e = entry("old");
        let id = e.id;
        s.push(e);
        s.update(TestEntry {
            id,
            key: "new".into(),
            valid: true,
        });
        assert_eq!(s.find(id).unwrap().key, "new");
        assert_eq!(s.entries.len(), 1);
    }

    #[test]
    fn update_on_missing_id_is_silent_no_op() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        s.push(entry("a"));
        s.update(TestEntry {
            id: Uuid::new_v4(),
            key: "ghost".into(),
            valid: true,
        });
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].key, "a");
    }

    #[test]
    fn remove_drops_only_matching_entry() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        let target = entry("a");
        let target_id = target.id;
        s.push(target);
        s.push(entry("b"));
        s.remove(target_id);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].key, "b");
    }

    #[test]
    fn remove_with_unknown_id_is_no_op() {
        let mut s: CatalogState<TestEntry> = CatalogState::empty(Uuid::new_v4());
        s.push(entry("a"));
        s.remove(Uuid::new_v4());
        assert_eq!(s.entries.len(), 1);
    }
}
