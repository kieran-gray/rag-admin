use uuid::Uuid;

use crate::shared::contracts::{MetadataFilterDto, QueryResult};

#[derive(Clone)]
pub(super) struct HistoryEntry {
    pub query: String,
    pub retrieval_profile_id: Uuid,
    pub profile_name: String,
    pub top_k: u32,
    pub min_score: f32,
    pub filters: Vec<MetadataFilterDto>,
    pub result: Option<QueryResult>,
    pub error: Option<String>,
    pub created_at_ms: f64,
}

#[cfg(feature = "hydrate")]
pub(super) fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn now_ms() -> f64 {
    0.0
}

pub(super) fn relative_time(now_ms: f64, then_ms: f64) -> String {
    if then_ms <= 0.0 || now_ms <= 0.0 {
        return String::new();
    }
    let diff = ((now_ms - then_ms).max(0.0) / 1000.0) as u64;
    if diff < 5 {
        return "just now".to_string();
    }
    if diff < 60 {
        return format!("{diff}s ago");
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86_400 {
        return format!("{}h ago", diff / 3600);
    }
    format!("{}d ago", diff / 86_400)
}
