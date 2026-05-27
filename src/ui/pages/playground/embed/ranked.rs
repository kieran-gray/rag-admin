use leptos::prelude::*;

use crate::shared::{EmbedInputType, EmbedManyResult};
use crate::ui::components::playground::LatencyBadge;
use crate::ui::components::primitives::{MetricBar, Surface};

use super::shared::{
    norm_tone, similarity_bucket, CharCount, InputTypeSelect, Stat, ThresholdLegend,
};

#[component]
pub(super) fn RankedInputs(
    query: RwSignal<String>,
    candidates: RwSignal<String>,
    query_type: RwSignal<EmbedInputType>,
    candidate_type: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <div class="embed-sources">
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Query"</span>
                    <InputTypeSelect value=query_type disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="What are you searching for?"
                    rows="4"
                    disabled=disabled
                    prop:value=move || query.get()
                    on:input=move |e| query.set(event_target_value(&e))
                ></textarea>
                <CharCount text=query />
            </label>
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Candidates (one per line)"</span>
                    <InputTypeSelect value=candidate_type disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="Paste one candidate per line…"
                    rows="10"
                    disabled=disabled
                    prop:value=move || candidates.get()
                    on:input=move |e| candidates.set(event_target_value(&e))
                ></textarea>
                <CharCount text=candidates />
            </label>
        </div>
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub(super) fn RankedResultPanel(
    result: EmbedManyResult,
    show_advanced: RwSignal<bool>,
) -> impl IntoView {
    let EmbedManyResult {
        dims,
        query_norm,
        candidates,
        timings,
    } = result;

    let count = candidates.len();
    let candidates_stored = StoredValue::new(candidates);

    view! {
        <Surface
            title=format!("Ranked · {count}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <ThresholdLegend />
                <ol class="embed-ranked-list">
                    {candidates_stored.with_value(|cs| cs.iter().enumerate().map(|(i, c)| {
                        let sim = c.similarity.clamp(0.0, 1.0);
                        let bucket = similarity_bucket(c.similarity);
                        let preview = c.text_preview.clone();
                        let norm = c.norm;
                        view! {
                            <li class="embed-ranked-row">
                                <div class="embed-ranked-rank">{i + 1}</div>
                                <div class="embed-ranked-body">
                                    <div class="embed-ranked-text">{preview}</div>
                                    <MetricBar
                                        label=format!("{:.3}", c.similarity)
                                        short=bucket.label.to_string()
                                        value=sim
                                        kind=bucket.kind
                                    />
                                </div>
                                <div class="embed-ranked-norm" title="‖vec‖">{format!("{norm:.3}")}</div>
                            </li>
                        }
                    }).collect_view())}
                </ol>

                <details class="embed-advanced" prop:open=move || show_advanced.get()>
                    <summary on:click=move |_| {
                        let next = !show_advanced.get_untracked();
                        show_advanced.set(next);
                    }>
                        "Advanced · dims, query norm"
                    </summary>
                    <div class="embed-stats">
                        <Stat label="Dimensions".to_string() value=dims.to_string() />
                        <Stat
                            label="‖query‖".to_string()
                            value=format!("{query_norm:.4}")
                            tone=norm_tone(query_norm)
                        />
                    </div>
                </details>
            </div>
        </Surface>
    }
}
