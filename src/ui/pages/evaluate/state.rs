use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::IndexProfileDto;

#[derive(Clone, Copy)]
pub struct EvaluateSelection {
    pub dataset_id: RwSignal<Option<Uuid>>,
    pub run_id: RwSignal<Option<Uuid>>,
    pub index_profile_id: RwSignal<Option<Uuid>>,
    pub just_launched: RwSignal<bool>,
}

impl EvaluateSelection {
    pub fn new(
        index_profiles: &[IndexProfileDto],
        initial_dataset: Option<Uuid>,
        initial_run: Option<Uuid>,
    ) -> Self {
        let initial_index_profile = index_profiles
            .iter()
            .find(|p| p.is_default)
            .or_else(|| index_profiles.first())
            .map(|p| p.index_profile_id);

        Self {
            dataset_id: RwSignal::new(initial_dataset),
            run_id: RwSignal::new(initial_run),
            index_profile_id: RwSignal::new(initial_index_profile),
            just_launched: RwSignal::new(false),
        }
    }
}
