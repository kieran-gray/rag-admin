use std::cmp::Ordering;
use std::collections::HashSet;

use leptos::prelude::*;

use crate::shared::{
    evaluation_score, pareto_frontier, EvaluationResultSplit, EvaluationVariantResult, ParetoPoint,
};
use crate::ui::components::primitives::Surface;

#[component]
pub(super) fn ParetoPanel(
    variants: Vec<EvaluationVariantResult>,
    bucket: Option<EvaluationResultSplit>,
) -> impl IntoView {
    let bucket_label = bucket
        .map(EvaluationResultSplit::as_str)
        .unwrap_or("analysis");

    let points: Vec<ParetoPoint> = variants
        .into_iter()
        .map(|v| ParetoPoint {
            quality: evaluation_score(&v.metrics),
            cost: v.metrics.average_retrieved_tokens as f32,
        })
        .collect();
    let frontier_idx: HashSet<usize> = pareto_frontier(&points).into_iter().collect();
    let champion_idx = points
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.quality.partial_cmp(&b.quality).unwrap_or(Ordering::Equal))
        .map(|(i, _)| i);

    const W: f32 = 480.0;
    const H: f32 = 200.0;
    const PAD_L: f32 = 40.0;
    const PAD_R: f32 = 16.0;
    const PAD_T: f32 = 12.0;
    const PAD_B: f32 = 28.0;
    let max_cost = points
        .iter()
        .map(|p| p.cost)
        .fold(0.0_f32, f32::max)
        .max(1.0);
    let to_x = move |cost: f32| PAD_L + (W - PAD_L - PAD_R) * (cost / max_cost).clamp(0.0, 1.0);
    let to_y = move |q: f32| PAD_T + (H - PAD_T - PAD_B) * (1.0 - q.clamp(0.0, 1.0));
    let view_box = format!("0 0 {W} {H}");

    view! {
        <Surface title=format!("Pareto frontier · {bucket_label}")>
            <div class="text-xs muted mb-3">
                "Quality (composite) vs. cost (mean retrieved tokens per question). Points outlined in accent are Pareto-optimal: nothing else gets you more quality for less cost. The ★ is the best-scoring config in this bucket. For optimization runs the bucket is the validation pass, where cheaper-but-tied alternatives to the champion live."
            </div>
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox=view_box
                style="width: 100%; max-width: 480px; height: auto;"
            >
                {[0.0_f32, 0.25, 0.5, 0.75, 1.0]
                    .iter()
                    .map(|q| {
                        let y = to_y(*q);
                        view! {
                            <line
                                x1=PAD_L
                                x2=W - PAD_R
                                y1=y
                                y2=y
                                stroke="var(--color-border)"
                                stroke-dasharray="2,4"
                            />
                        }
                    })
                    .collect_view()}
                <line
                    x1=PAD_L
                    x2=PAD_L
                    y1=PAD_T
                    y2=H - PAD_B
                    stroke="var(--color-border-strong)"
                />
                <line
                    x1=PAD_L
                    x2=W - PAD_R
                    y1=H - PAD_B
                    y2=H - PAD_B
                    stroke="var(--color-border-strong)"
                />
                {points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let cx = to_x(p.cost);
                        let cy = to_y(p.quality);
                        let on_frontier = frontier_idx.contains(&i);
                        let is_champion = champion_idx == Some(i);
                        let fill = if is_champion {
                            "var(--color-accent)"
                        } else if on_frontier {
                            "var(--color-accent-soft)"
                        } else {
                            "var(--color-text-muted)"
                        };
                        let stroke = if on_frontier || is_champion {
                            "var(--color-accent)"
                        } else {
                            "var(--color-border-strong)"
                        };
                        let r = if is_champion { 6.0 } else { 4.0 };
                        view! {
                            <circle
                                cx=cx
                                cy=cy
                                r=r
                                fill=fill
                                stroke=stroke
                                stroke-width="1.5"
                            />
                        }
                    })
                    .collect_view()}
                <text
                    x=PAD_L - 6.0
                    y=PAD_T + 6.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="end"
                >
                    "100%"
                </text>
                <text
                    x=PAD_L - 6.0
                    y=H - PAD_B + 3.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="end"
                >
                    "0%"
                </text>
                <text
                    x=W / 2.0
                    y=H - 6.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                >
                    {format!("cost → (max ≈ {max_cost:.0})")}
                </text>
            </svg>
        </Surface>
    }
}
