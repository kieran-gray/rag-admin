use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{ChunkingConfigurationDto, IndexProfileDto, IndexingDto};

#[derive(Clone, Copy)]
pub struct ConfigSelection {
    pub index_profile_id: RwSignal<Option<Uuid>>,
    pub chunking_id: RwSignal<Option<Uuid>>,
}

impl ConfigSelection {
    pub fn new(
        index_profiles: &[IndexProfileDto],
        chunking: &[ChunkingConfigurationDto],
        indexings: &[IndexingDto],
    ) -> Self {
        let initial_index_profile = indexings
            .iter()
            .find(|i| !i.removed)
            .map(|i| i.index_profile_id)
            .or_else(|| {
                index_profiles
                    .iter()
                    .find(|p| p.is_default)
                    .or_else(|| index_profiles.first())
                    .map(|p| p.index_profile_id)
            });

        let initial_chunking = chunking
            .iter()
            .find(|c| c.is_default)
            .or_else(|| chunking.first())
            .map(|c| c.chunking_configuration_id);

        Self {
            index_profile_id: RwSignal::new(initial_index_profile),
            chunking_id: RwSignal::new(initial_chunking),
        }
    }
}
