use leptos::prelude::*;

use crate::shared::{EvaluationResultSplit, EvaluationScoreWeights, EvaluationVariantResult};
use crate::ui::components::primitives::Surface;

use super::shared::METRIC_DEFS;

#[derive(Clone)]
pub(super) struct RetrievalSummary {
    pub headline: String,
    pub detail: String,
    pub tooltip: String,
}

pub(super) fn summarise_retrieval(variants: &[EvaluationVariantResult]) -> RetrievalSummary {
    if variants.is_empty() {
        return RetrievalSummary {
            headline: "—".into(),
            detail: "No variants".into(),
            tooltip: String::new(),
        };
    }

    let mut top_ks: Vec<u32> = variants.iter().map(|v| v.options.top_k).collect();
    top_ks.sort_unstable();
    top_ks.dedup();

    let mut min_scores_milli: Vec<u32> =
        variants.iter().map(|v| v.options.min_score_milli).collect();
    min_scores_milli.sort_unstable();
    min_scores_milli.dedup();

    let format_top_ks = |slice: &[u32]| {
        slice
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let format_min_scores = |slice: &[u32]| {
        slice
            .iter()
            .map(|m| format!("{:.2}", (*m as f32) / 1000.0))
            .collect::<Vec<_>>()
            .join(", ")
    };

    const INLINE_LIMIT: usize = 3;

    let top_k_part = match top_ks.as_slice() {
        [k] => format!("topK {k}"),
        ks if ks.len() <= INLINE_LIMIT => format!("topK {{{}}}", format_top_ks(ks)),
        ks => format!(
            "topK {} to {} ({} values)",
            ks.first().copied().unwrap_or(0),
            ks.last().copied().unwrap_or(0),
            ks.len(),
        ),
    };
    let min_part = match min_scores_milli.as_slice() {
        [m] => format!("min {:.2}", (*m as f32) / 1000.0),
        ms if ms.len() <= INLINE_LIMIT => format!("min {{{}}}", format_min_scores(ms)),
        ms => format!(
            "min {:.2} to {:.2} ({} values)",
            (*ms.first().unwrap_or(&0) as f32) / 1000.0,
            (*ms.last().unwrap_or(&0) as f32) / 1000.0,
            ms.len(),
        ),
    };

    let combinations = top_ks.len() * min_scores_milli.len();
    let detail = if combinations <= 1 {
        "Shared across all variants".to_string()
    } else {
        format!("Swept across {combinations} combinations")
    };

    let tooltip = if top_ks.len() > INLINE_LIMIT || min_scores_milli.len() > INLINE_LIMIT {
        format!(
            "topK: {}\nmin: {}",
            format_top_ks(&top_ks),
            format_min_scores(&min_scores_milli),
        )
    } else {
        String::new()
    };

    RetrievalSummary {
        headline: format!("{top_k_part} · {min_part}"),
        detail,
        tooltip,
    }
}

#[component]
pub(super) fn RunSummary(
    leader_score: Option<f32>,
    leader_ci_half: Option<f32>,
    leader_split: Option<EvaluationResultSplit>,
    variants_count: usize,
    retrieval: RetrievalSummary,
) -> impl IntoView {
    let weights = EvaluationScoreWeights::default();
    let score_str = leader_score
        .map(|s| format!("{:.1}%", s * 100.0))
        .unwrap_or_else(|| "—".to_string());
    let ci_str = leader_ci_half.map(|h| format!(" ± {:.1}", h * 100.0));
    let eyebrow = match leader_split {
        Some(EvaluationResultSplit::Holdout) => "Leader score · holdout".to_string(),
        Some(EvaluationResultSplit::Validation) => "Leader score · validation".to_string(),
        Some(EvaluationResultSplit::Tuning) => "Leader score · tuning (preliminary)".to_string(),
        Some(EvaluationResultSplit::Full) | None => "Leader score".to_string(),
    };
    let footnote = match leader_split {
        Some(EvaluationResultSplit::Holdout) => {
            "Final integrity pass on held-out questions, single touch.".to_string()
        }
        Some(EvaluationResultSplit::Validation) => {
            "Validation pass; chooses the champion before the holdout touch.".to_string()
        }
        Some(EvaluationResultSplit::Tuning) => {
            "Optimizer rung; preliminary, not the deployment number.".to_string()
        }
        _ => "Weighted composite ± 95% bootstrap confidence interval half-width".to_string(),
    };
    view! {
        <Surface flush=true>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-px bg-[var(--color-border)]">
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Variants compared"</div>
                    <div class="text-lg mt-1 font-mono">{variants_count}</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">{eyebrow}</div>
                    <div class="text-lg mt-1 font-mono text-[var(--color-accent)]">
                        {score_str}
                        {ci_str.map(|s| view! { <span class="text-sm muted">{s}</span> })}
                    </div>
                    <div class="text-xs muted mt-1">{footnote}</div>
                </div>
                <div
                    class="bg-[var(--color-surface-1)] p-4"
                    title=retrieval.tooltip
                >
                    <div class="eyebrow">"Retrieval"</div>
                    <div class="text-lg mt-1 font-mono">{retrieval.headline}</div>
                    <div class="text-xs muted mt-1">{retrieval.detail}</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Score weights"</div>
                    <div class="text-xs mt-1 muted font-mono">
                        {format!(
                            "Recall {:.0}% · IoU {:.0}% · Precision {:.0}% · Pω {:.0}%",
                            weights.recall * 100.0,
                            weights.iou * 100.0,
                            weights.precision * 100.0,
                            weights.precision_omega * 100.0,
                        )}
                    </div>
                </div>
            </div>
        </Surface>
    }
}

#[component]
pub(super) fn MetricLegend() -> impl IntoView {
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
pub(super) fn AxisLegend() -> impl IntoView {
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
