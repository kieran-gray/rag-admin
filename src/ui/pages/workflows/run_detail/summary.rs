use leptos::prelude::*;

use crate::shared::{EvaluationResultSplit, EvaluationScoreWeights};
use crate::ui::components::primitives::Surface;

use super::advisor::AdvisorEntry;
use super::shared::METRIC_DEFS;

pub(crate) struct RunScoreData {
    pub score: f32,
    pub ci_half: Option<f32>,
    pub split: EvaluationResultSplit,
    pub variant_label: String,
    pub top_k: u32,
    pub min_score: f32,
    pub question_count: u32,
}

#[component]
pub(crate) fn RunScoreCard(
    data: Option<RunScoreData>,
    tied_count: usize,
    on_view_ties: Callback<()>,
) -> impl IntoView {
    let Some(d) = data else {
        return view! {
            <div class="run-score-card run-score-card-empty">
                <div class="eyebrow">"Run score"</div>
                <div class="run-score-value muted">"—"</div>
                <div class="muted text-xs mt-1">"No variants scored yet."</div>
            </div>
        }
        .into_any();
    };

    let (phase_label, phase_help) = match d.split {
        EvaluationResultSplit::Holdout => (
            "Holdout".to_string(),
            "Final integrity pass on held-out questions.".to_string(),
        ),
        EvaluationResultSplit::Validation => (
            "Validation".to_string(),
            "Validation pass; chose the champion before the holdout touch.".to_string(),
        ),
        EvaluationResultSplit::Tuning => (
            "Tuning".to_string(),
            "Optimizer rung — preliminary, not the deployment number.".to_string(),
        ),
        EvaluationResultSplit::Full => (
            "Full".to_string(),
            "Scored on the full dataset.".to_string(),
        ),
    };

    let ci_str = d.ci_half.map(|h| format!(" ± {:.1}pp", h * 100.0));

    view! {
        <div class="run-score-card">
            <div class="eyebrow">"Run score"</div>
            <div class="run-score-value">
                {format!("{:.1}%", d.score * 100.0)}
                {ci_str.map(|s| view! { <span class="run-score-ci">{s}</span> })}
            </div>
            <div class="run-score-chips">
                <span class="pill pill-accent" title=phase_help>{phase_label}</span>
                <span class="muted text-xs">{format!("{} questions", d.question_count)}</span>
            </div>
            <div class="run-score-config">
                <span class="font-mono">{d.variant_label}</span>
                <span class="muted text-xs">
                    {format!(" · topK {} · min {:.2}", d.top_k, d.min_score)}
                </span>
            </div>
            {(tied_count > 1).then(|| view! {
                <button
                    type="button"
                    class="run-score-ties"
                    on:click=move |_| on_view_ties.run(())
                    title="Configs whose 95% CI overlaps the leader's — ranking between them is not reliable at this dataset size."
                >
                    {format!("≡ {} statistical ties", tied_count - 1)}
                    <span class="muted">" · view"</span>
                </button>
            })}
        </div>
    }
    .into_any()
}

#[component]
pub(crate) fn NotesCard(
    advisor: Vec<AdvisorEntry>,
    variants_count: usize,
    weights: EvaluationScoreWeights,
) -> impl IntoView {
    view! {
        <div class="run-notes-card">
            <div class="eyebrow">"Notes"</div>
            <ul class="run-notes-list">
                <li class="run-notes-row">
                    <span class="run-notes-bullet" aria-hidden="true">"●"</span>
                    <span>
                        {format!("{variants_count} variants compared")}
                    </span>
                </li>
                <li class="run-notes-row">
                    <span class="run-notes-bullet" aria-hidden="true">"●"</span>
                    <span class="font-mono text-xs">
                        {format!(
                            "Weights · R {:.0}% / IoU {:.0}% / P {:.0}% / Pω {:.0}%",
                            weights.recall * 100.0,
                            weights.iou * 100.0,
                            weights.precision * 100.0,
                            weights.precision_omega * 100.0,
                        )}
                    </span>
                </li>
                {advisor.into_iter().map(|e| view! {
                    <li class="run-notes-row run-notes-warn">
                        <span class="run-notes-bullet" aria-hidden="true">"⚠"</span>
                        <span>
                            <span class="run-notes-headline">{e.flag.headline()}</span>
                            <span class="muted text-xs">" — "</span>
                            <span class="text-xs">{e.detail}</span>
                        </span>
                    </li>
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
pub(crate) fn RunScoreStrip(
    score: Option<RunScoreData>,
    tied_count: usize,
    advisor: Vec<AdvisorEntry>,
    variants_count: usize,
    weights: EvaluationScoreWeights,
    on_view_ties: Callback<()>,
) -> impl IntoView {
    view! {
        <Surface flush=true>
            <div class="run-score-strip">
                <RunScoreCard data=score tied_count=tied_count on_view_ties=on_view_ties />
                <NotesCard advisor=advisor variants_count=variants_count weights=weights />
            </div>
        </Surface>
    }
}

#[component]
pub(crate) fn MetricLegend() -> impl IntoView {
    view! {
        <div class="my-4">
            <details>
                <summary class="text-xs muted cursor-pointer hover:text-text">
                    "What do these metrics mean?"
                </summary>
                <div class="mt-3 p-4 surface-raised rounded text-sm space-y-2.5">
                    {METRIC_DEFS.iter().map(|d| view! {
                        <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                            <span class="font-medium">{d.name}</span>
                            <span class="text-xs muted font-mono">{d.short}</span>
                            <span class="muted">{d.help}</span>
                        </div>
                    }).collect_view()}
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline pt-2 border-t border-[var(--color-border)]">
                        <span class="font-medium">"± marker"</span>
                        <span class="text-xs muted font-mono">"95% CI"</span>
                        <span class="muted">
                            "95% bootstrap confidence interval half-width on the mean. A measure of statistical uncertainty given this dataset size. Two variants whose intervals overlap can't be ranked reliably at this dataset size."
                        </span>
                    </div>
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                        <span class="font-medium">"Pink tick"</span>
                        <span class="text-xs muted font-mono">"▎"</span>
                        <span class="muted">
                            "Best score for that metric across all variants in the run. Lets you see how close a runner-up is to the leader on each dimension."
                        </span>
                    </div>
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                        <span class="font-medium">"Judge†"</span>
                        <span class="text-xs muted font-mono">"qual"</span>
                        <span class="muted">
                            "Qualitative LLM-judge diagnostic across a 5-question sample. Useful as a sanity check on whether the retrieved context looks sufficient; the sample is too small to be a primary score and is not used by the optimizer."
                        </span>
                    </div>
                </div>
            </details>
        </div>
    }
}

#[component]
pub(crate) fn AxisLegend() -> impl IntoView {
    view! {
        <div class="metric-bar-axis mt-4 pt-3 border-t border-[var(--color-border)]">
            <span></span>
            <div class="metric-bar-axis-scale">
                <span>"0%"</span>
                <span style="left: 25%">"25%"</span>
                <span style="left: 50%">"50%"</span>
                <span style="left: 75%">"75%"</span>
                <span style="left: 100%">"100%"</span>
            </div>
            <span></span>
        </div>
    }
}
