use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ui::components::primitives::Surface;

use super::shared::preview;

#[cfg(feature = "hydrate")]
const PAIRWISE_HISTORY_KEY: &str = "rag-admin:embed-history";
pub(super) const PAIRWISE_HISTORY_LIMIT: usize = 8;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct PairwiseHistoryEntry {
    pub model: String,
    pub text_a: String,
    pub text_b: String,
    pub similarity: f32,
}

#[cfg(feature = "hydrate")]
pub(super) fn load_pairwise_history() -> Vec<PairwiseHistoryEntry> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Vec::new();
    };
    let Ok(Some(raw)) = storage.get_item(PAIRWISE_HISTORY_KEY) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(feature = "hydrate")]
pub(super) fn save_pairwise_history(entries: &[PairwiseHistoryEntry]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if entries.is_empty() {
        drop(storage.remove_item(PAIRWISE_HISTORY_KEY));
        return;
    }
    if let Ok(json) = serde_json::to_string(entries) {
        drop(storage.set_item(PAIRWISE_HISTORY_KEY, &json));
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn load_pairwise_history() -> Vec<PairwiseHistoryEntry> {
    Vec::new()
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn save_pairwise_history(_entries: &[PairwiseHistoryEntry]) {}

#[component]
pub(super) fn HistorySidebar(
    history: RwSignal<Vec<PairwiseHistoryEntry>>,
    on_select: impl Fn(PairwiseHistoryEntry) + Copy + Send + Sync + 'static,
    on_clear: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <Surface
            title="Recent pairs".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-sm btn-ghost"
                    disabled=move || history.with(Vec::is_empty)
                    on:click=move |_| on_clear()
                >
                    "Clear"
                </button>
            }.into_any())
        >
            {move || {
                let h = history.get();
                if h.is_empty() {
                    view! {
                        <p class="muted text-sm">"No history yet."</p>
                    }.into_any()
                } else {
                    h.into_iter().map(|entry| {
                        let entry_for_click = entry.clone();
                        let preview_a = preview(&entry.text_a, 40);
                        let preview_b = preview(&entry.text_b, 40);
                        let sim = format!("{:.3}", entry.similarity);
                        view! {
                            <button
                                type="button"
                                class="playground-history-row"
                                on:click=move |_| on_select(entry_for_click.clone())
                            >
                                <div class="playground-history-query">
                                    <span class="embed-history-sim">{sim}</span>
                                    " "
                                    {preview_a}
                                </div>
                                <div class="playground-history-meta">
                                    <span>{preview_b}</span>
                                </div>
                            </button>
                        }
                    }).collect_view().into_any()
                }
            }}
        </Surface>
    }
}
