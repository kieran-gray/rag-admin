use leptos::prelude::*;

use crate::core::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::ui::components::primitives::Surface;

use super::shared::{composite_ci_bounds, variant_display};

#[component]
pub(super) fn EquivalenceClassPanel(
    members: Vec<EvaluationVariantResult>,
    bucket: Option<EvaluationResultSplit>,
) -> impl IntoView {
    let count = members.len();
    let bucket_label = bucket
        .map(EvaluationResultSplit::as_str)
        .unwrap_or("analysis");
    view! {
        <Surface
            title=format!("Possible ties · {count} configs overlap on {bucket_label}")
        >
            <div class="text-xs muted mb-3">
                "These configs' composite-score 95% confidence intervals overlap the leader's in this bucket. Confidence-interval overlap is a rough heuristic, not a formal equivalence test, but the optimizer's ranking between them is unlikely to be reliable at this dataset size. Prefer the cheapest or simplest unless you have a reason."
            </div>
            <table class="variants-table">
                <thead>
                    <tr>
                        <th>"Variant"</th>
                        <th class="num">"TopK"</th>
                        <th class="num">"Min score"</th>
                        <th class="num">"Score"</th>
                        <th class="num" title="95% bootstrap confidence interval on the composite score">"95% confidence interval"</th>
                    </tr>
                </thead>
                <tbody>
                    {members.into_iter().enumerate().map(|(i, v)| {
                        let score = evaluation_score(&v.metrics);
                        let (lo, hi) = composite_ci_bounds(&v);
                        let ci_label = if hi > lo {
                            format!("{:.1}% – {:.1}%", lo * 100.0, hi * 100.0)
                        } else {
                            "—".into()
                        };
                        let (headline, trial_tag) = variant_display(&v);
                        view! {
                            <tr class=if i == 0 { "is-leader" } else { "" }>
                                <td>
                                    <span class="flex items-center gap-2">
                                        {(i == 0).then(|| view! {
                                            <span class="text-[var(--color-accent)]" title="Leader">"★"</span>
                                        })}
                                        <span class="font-mono">{headline}</span>
                                        {trial_tag.map(|t| view! {
                                            <span class="pill pill-neutral font-mono" title="Optimizer trial id">{t}</span>
                                        })}
                                    </span>
                                </td>
                                <td class="num">{v.options.top_k}</td>
                                <td class="num">{format!("{:.2}", v.options.min_score())}</td>
                                <td class="num"><strong>{format!("{:.1}%", score * 100.0)}</strong></td>
                                <td class="num muted">{ci_label}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </Surface>
    }
}
