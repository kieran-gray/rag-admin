use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

pub struct Registry<K, T: ?Sized> {
    items: HashMap<K, Arc<T>>,
}

impl<K: Eq + Hash, T: ?Sized> Registry<K, T> {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn register(&mut self, key: K, item: Arc<T>) {
        self.items.insert(key, item);
    }

    pub fn get(&self, key: &K) -> Option<&Arc<T>> {
        self.items.get(key)
    }
}

impl<K: Eq + Hash, T: ?Sized> Default for Registry<K, T> {
    fn default() -> Self {
        Self::new()
    }
}
