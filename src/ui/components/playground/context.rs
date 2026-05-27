use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::MetadataFilterDto;

#[derive(Clone, Copy)]
pub struct PlaygroundContext {
    pub last_query: RwSignal<String>,
    pub last_profile: RwSignal<Option<Uuid>>,
    pub last_filters: RwSignal<Vec<MetadataFilterDto>>,
}

impl Default for PlaygroundContext {
    fn default() -> Self {
        Self {
            last_query: RwSignal::new(String::new()),
            last_profile: RwSignal::new(None),
            last_filters: RwSignal::new(Vec::new()),
        }
    }
}

pub fn provide_playground_context() {
    provide_context(PlaygroundContext::default());
}

pub fn use_playground_context() -> PlaygroundContext {
    use_context::<PlaygroundContext>().unwrap_or_default()
}
