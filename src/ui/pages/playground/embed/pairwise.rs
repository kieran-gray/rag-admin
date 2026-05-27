use leptos::prelude::*;

use crate::shared::{EmbedInputType, EmbedResult};
use crate::ui::components::playground::LatencyBadge;
use crate::ui::components::primitives::{MetricBar, Surface};

use super::shared::{
    norm_tone, norm_warn, similarity_bucket, CharCount, InputTypeSelect, Stat, ThresholdLegend,
};

#[component]
pub(super) fn PairwiseInputs(
    text_a: RwSignal<String>,
    text_b: RwSignal<String>,
    type_a: RwSignal<EmbedInputType>,
    type_b: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
    on_swap: impl Fn(()) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="embed-sources">
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Text A"</span>
                    <InputTypeSelect value=type_a disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="First text segment…"
                    rows="8"
                    disabled=disabled
                    prop:value=move || text_a.get()
                    on:input=move |e| text_a.set(event_target_value(&e))
                ></textarea>
                <CharCount text=text_a />
            </label>
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Text B"</span>
                    <InputTypeSelect value=type_b disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="Second text segment…"
                    rows="8"
                    disabled=disabled
                    prop:value=move || text_b.get()
                    on:input=move |e| text_b.set(event_target_value(&e))
                ></textarea>
                <CharCount text=text_b />
            </label>
        </div>
        <div class="embed-swap-row">
            <button
                type="button"
                class="btn btn-sm btn-ghost"
                title="Swap A ↔ B (useful for asymmetric models)"
                disabled=disabled
                on:click=move |_| on_swap(())
            >
                "Swap A ↔ B"
            </button>
        </div>
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub(super) fn PairwiseResultPanel(
    result: EmbedResult,
    show_advanced: RwSignal<bool>,
) -> impl IntoView {
    let EmbedResult {
        dims,
        norm_a,
        norm_b,
        similarity,
        timings,
    } = result;

    let similarity_clamped = similarity.clamp(0.0, 1.0);
    let bucket = similarity_bucket(similarity);
    let norm_warns = norm_warn(norm_a) || norm_warn(norm_b);
    if norm_warns {
        show_advanced.set(true);
    }

    view! {
        <Surface
            title=format!("Similarity · {similarity:.3}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <MetricBar
                    label="Cosine similarity".to_string()
                    short=bucket.label.to_string()
                    value=similarity_clamped
                    kind=bucket.kind
                />
                <ThresholdLegend />

                <details class="embed-advanced" prop:open=move || show_advanced.get()>
                    <summary on:click=move |_| {
                        let next = !show_advanced.get_untracked();
                        show_advanced.set(next);
                    }>
                        "Advanced · dims, norms"
                    </summary>
                    <div class="embed-stats">
                        <Stat label="Dimensions".to_string() value=dims.to_string() />
                        <Stat label="‖A‖".to_string() value=format!("{norm_a:.4}") tone=norm_tone(norm_a) />
                        <Stat label="‖B‖".to_string() value=format!("{norm_b:.4}") tone=norm_tone(norm_b) />
                    </div>
                    {norm_warns.then(|| view! {
                        <p class="text-xs faint mt-3">
                            "Well-behaved models normalize to ~1.0; large deviations suggest a model or transport mismatch."
                        </p>
                    })}
                </details>
            </div>
        </Surface>
    }
}
