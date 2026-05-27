use leptos::prelude::*;

use crate::shared::contracts::MetadataFilterDto;
use crate::ui::components::playground::MetadataFilters;

#[component]
pub fn RetrievalControls(
    top_k: RwSignal<u32>,
    min_score: RwSignal<f32>,
    filters: RwSignal<Vec<MetadataFilterDto>>,
) -> impl IntoView {
    view! {
        <div class="retrieval-controls">
            <label class="retrieval-control">
                <span>"Top-K"</span>
                <input
                    type="number"
                    min="1"
                    max="50"
                    prop:value=move || top_k.get().to_string()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                            top_k.set(v.clamp(1, 50));
                        }
                    }
                />
            </label>
            <label class="retrieval-control">
                <span>"Min score"</span>
                <input
                    type="number"
                    min="0"
                    max="1"
                    step="0.05"
                    prop:value=move || format!("{:.2}", min_score.get())
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                            min_score.set(v.clamp(0.0, 1.0));
                        }
                    }
                />
            </label>
            <span class="retrieval-controls-divider" aria-hidden="true"></span>
            <div class="retrieval-controls-filters">
                <MetadataFilters filters=filters />
            </div>
        </div>
    }
}
