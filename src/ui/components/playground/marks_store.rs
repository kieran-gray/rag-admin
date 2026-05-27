use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkKind {
    Relevant,
    Irrelevant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub query: String,
    pub chunk_id: Uuid,
    pub kind: MarkKind,
}

pub fn next_kind(current: Option<MarkKind>) -> Option<MarkKind> {
    match current {
        None => Some(MarkKind::Relevant),
        Some(MarkKind::Relevant) => Some(MarkKind::Irrelevant),
        Some(MarkKind::Irrelevant) => None,
    }
}

#[cfg(feature = "hydrate")]
const STORAGE_PREFIX: &str = "rag-admin:marks";

#[cfg(feature = "hydrate")]
fn storage_key(profile_id: Uuid) -> String {
    format!("{STORAGE_PREFIX}:{profile_id}")
}

#[cfg(feature = "hydrate")]
pub fn load(profile_id: Uuid) -> Vec<Mark> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Vec::new();
    };
    let Ok(Some(raw)) = storage.get_item(&storage_key(profile_id)) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(feature = "hydrate")]
pub fn save(profile_id: Uuid, marks: &[Mark]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if marks.is_empty() {
        drop(storage.remove_item(&storage_key(profile_id)));
        return;
    }
    if let Ok(json) = serde_json::to_string(marks) {
        drop(storage.set_item(&storage_key(profile_id), &json));
    }
}

#[cfg(not(feature = "hydrate"))]
pub fn load(_profile_id: Uuid) -> Vec<Mark> {
    Vec::new()
}

#[cfg(not(feature = "hydrate"))]
pub fn save(_profile_id: Uuid, _marks: &[Mark]) {}
