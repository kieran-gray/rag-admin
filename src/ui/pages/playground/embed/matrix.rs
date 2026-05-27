use leptos::prelude::*;

use crate::shared::{EmbedInputType, EmbedMatrixResult};
use crate::ui::components::playground::LatencyBadge;
use crate::ui::components::primitives::Surface;

use super::shared::{CharCount, InputTypeSelect};

#[component]
pub(super) fn MatrixInputs(
    texts: RwSignal<String>,
    input_type: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="embed-source-field">
            <div class="embed-source-head">
                <span class="eyebrow">"Texts (one per line, max 16)"</span>
                <InputTypeSelect value=input_type disabled=disabled />
            </div>
            <textarea
                class="playground-query-input"
                placeholder="Paste one text per line…"
                rows="10"
                disabled=disabled
                prop:value=move || texts.get()
                on:input=move |e| texts.set(event_target_value(&e))
            ></textarea>
            <CharCount text=texts />
        </label>
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub(super) fn MatrixResultPanel(result: EmbedMatrixResult) -> impl IntoView {
    let EmbedMatrixResult {
        dims,
        previews,
        norms,
        matrix,
        timings,
    } = result;

    let n = previews.len();
    let matrix_stored = StoredValue::new(matrix);
    let previews_stored = StoredValue::new(previews.clone());
    let norms_stored = StoredValue::new(norms);

    view! {
        <Surface
            title=format!("Matrix · {n}x{n}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <div
                    class="embed-matrix"
                    style=format!(
                        "grid-template-columns: minmax(140px, 1fr) repeat({n}, minmax(36px, 1fr));"
                    )
                >
                    <div class="embed-matrix-corner"></div>
                    {(0..n).map(|j| view! {
                        <div class="embed-matrix-col-head">{j + 1}</div>
                    }).collect_view()}

                    {(0..n).map(|i| {
                        let preview = previews_stored.with_value(|p| p.get(i).cloned().unwrap_or_default());
                        let norm = norms_stored.with_value(|ns| ns.get(i).copied().unwrap_or_default());
                        let row_head = view! {
                            <div class="embed-matrix-row-head" title=format!("‖vec‖ {norm:.3}")>
                                <span class="embed-matrix-row-rank">{i + 1}</span>
                                <span class="embed-matrix-row-text">{preview}</span>
                            </div>
                        };
                        let cells = (0..n).map(|j| {
                            let sim = matrix_stored.with_value(|m| {
                                m.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0)
                            });
                            let bg = matrix_cell_bg(sim);
                            view! {
                                <div
                                    class="embed-matrix-cell"
                                    style=format!("background-color: {bg};")
                                    title=format!("({}, {}) {sim:.3}", i + 1, j + 1)
                                >
                                    {format!("{sim:.2}")}
                                </div>
                            }
                        }).collect_view();
                        view! { <>{row_head}{cells}</> }
                    }).collect_view()}
                </div>

                <p class="text-xs faint mt-3">
                    {format!("Dimensions: {dims}")}
                </p>
            </div>
        </Surface>
    }
}

fn matrix_cell_bg(sim: f32) -> String {
    let s = sim.clamp(0.0, 1.0);
    let alpha = 0.08_f32 + 0.7_f32 * s;
    format!("rgba(247, 118, 142, {alpha:.3})")
}
