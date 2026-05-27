use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use leptos::prelude::*;

use crate::shared::contracts::RunKindDto;
use crate::shared::{evaluation_score, EvaluationVariantResult};
use crate::ui::components::primitives::{Status, StatusPill};

pub fn optimizer_section_available(kind: RunKindDto, variants: &[EvaluationVariantResult]) -> bool {
    if kind == RunKindDto::Optimization {
        return true;
    }
    variants
        .iter()
        .any(|v| v.variant.label.starts_with("trial-"))
}

#[component]
pub fn OptimizerSection(
    variants: Vec<EvaluationVariantResult>,
    is_running: bool,
    default_open: bool,
) -> impl IntoView {
    let rungs = rung_summary(&variants);
    let variants_for_chart = variants;

    view! {
        <details class="run-collapsible" open=default_open>
            <summary class="run-collapsible-summary">
                <span class="run-collapsible-chevron" aria-hidden="true">"▸"</span>
                <span class="run-collapsible-label">"Optimizer progress"</span>
                {is_running.then(|| view! {
                    <StatusPill label="Updating live".to_string() kind=Status::Pending />
                })}
                <span class="muted text-xs">
                    "Best-so-far curve and rung structure. Useful while the run is converging."
                </span>
            </summary>
            <div class="run-collapsible-body space-y-6">
                {(!variants_for_chart.is_empty()).then(|| view! {
                    <BestSoFarChart trials=variants_for_chart.clone() />
                })}
                {(!rungs.is_empty()).then(|| view! { <RungSummaryPanel rows=rungs /> })}
            </div>
        </details>
    }
}

#[component]
fn BestSoFarChart(trials: Vec<EvaluationVariantResult>) -> impl IntoView {
    let mut ordered = trials;
    ordered.sort_by(|a, b| a.variant.label.cmp(&b.variant.label));
    let mut best: Vec<(f32, f32, f32)> = Vec::with_capacity(ordered.len());
    let mut running = f32::MIN;
    let mut running_lo = 0.0f32;
    let mut running_hi = 0.0f32;
    for v in &ordered {
        let s = evaluation_score(&v.metrics);
        if s > running {
            running = s;
            running_lo = v.metrics.composite_ci_low;
            running_hi = v.metrics.composite_ci_high;
        }
        best.push((running.max(0.0), running_lo.max(0.0), running_hi.max(0.0)));
    }
    if best.is_empty() {
        return view! { <p class="muted">"No trials yet."</p> }.into_any();
    }

    const W: f32 = 720.0;
    const H: f32 = 180.0;
    const PAD_L: f32 = 36.0;
    const PAD_R: f32 = 12.0;
    const PAD_T: f32 = 10.0;
    const PAD_B: f32 = 24.0;

    let n = best.len() as f32;
    let to_x = move |i: usize| {
        if n <= 1.0 {
            PAD_L + (W - PAD_L - PAD_R) / 2.0
        } else {
            PAD_L + (W - PAD_L - PAD_R) * (i as f32) / (n - 1.0)
        }
    };
    let to_y = |s: f32| PAD_T + (H - PAD_T - PAD_B) * (1.0 - s.clamp(0.0, 1.0));

    let line_path = best
        .iter()
        .enumerate()
        .map(|(i, (s, _, _))| {
            let cmd = if i == 0 { "M" } else { "L" };
            format!("{cmd} {:.1} {:.1}", to_x(i), to_y(*s))
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut band_path = String::new();
    if best.iter().any(|(_, lo, hi)| hi > lo) {
        for (i, (_, _, hi)) in best.iter().enumerate() {
            let cmd = if i == 0 { "M" } else { "L" };
            _ = write!(band_path, "{cmd} {:.1} {:.1} ", to_x(i), to_y(*hi));
        }
        for (i, (_, lo, _)) in best.iter().enumerate().rev() {
            _ = write!(band_path, "L {:.1} {:.1} ", to_x(i), to_y(*lo));
        }
        band_path.push('Z');
    }

    let svg_view_box = format!("0 0 {W} {H}");
    let final_score = best.last().map(|(s, _, _)| *s).unwrap_or(0.0);

    view! {
        <div>
            <div class="muted text-xs mb-2">
                "Running best composite across trials, with the leader's 95% CI as a band. The plateau tells you when the optimizer stopped finding improvements."
            </div>
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox=svg_view_box
                style="width: 100%; max-width: 720px; height: auto;"
            >
                {[(0.0, "0%"), (0.25, "25%"), (0.5, "50%"), (0.75, "75%"), (1.0, "100%")]
                    .iter()
                    .map(|(v, label)| {
                        let y = to_y(*v);
                        view! {
                            <g>
                                <line
                                    x1=PAD_L
                                    x2=W - PAD_R
                                    y1=y
                                    y2=y
                                    stroke="var(--color-border)"
                                    stroke-dasharray="2,4"
                                />
                                <text
                                    x=PAD_L - 6.0
                                    y=y + 3.0
                                    font-size="10"
                                    fill="var(--color-text-muted)"
                                    text-anchor="end"
                                >
                                    {*label}
                                </text>
                            </g>
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
                {(!band_path.is_empty()).then(|| view! {
                    <path
                        d=band_path
                        fill="var(--color-accent)"
                        fill-opacity="0.15"
                        stroke="none"
                    />
                })}
                <path
                    d=line_path
                    fill="none"
                    stroke="var(--color-accent)"
                    stroke-width="2"
                />
                <text
                    x=W / 2.0
                    y=H - 4.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                >
                    {format!("Trial 1 … Trial {}  ·  best = {:.1}%", best.len(), final_score * 100.0)}
                </text>
            </svg>
        </div>
    }
    .into_any()
}

fn parse_trial_rung(label: &str) -> Option<(u32, u32)> {
    let rest = label.strip_prefix("trial-")?;
    let (trial_str, rung_str) = rest.split_once("-r")?;
    let trial_id: u32 = trial_str.parse().ok()?;
    let rung: u32 = rung_str.parse().ok()?;
    Some((trial_id, rung))
}

#[derive(Clone)]
struct RungRow {
    rung: u32,
    trials: usize,
    mean_composite: f32,
    survivors: Option<usize>,
}

fn rung_summary(variants: &[EvaluationVariantResult]) -> Vec<RungRow> {
    let mut by_rung: BTreeMap<u32, (Vec<f32>, HashSet<u32>)> = BTreeMap::new();
    let mut max_rung_by_trial: HashMap<u32, u32> = HashMap::new();

    for v in variants {
        let Some((trial_id, rung)) = parse_trial_rung(&v.variant.label) else {
            continue;
        };
        let entry = by_rung.entry(rung).or_default();
        entry.0.push(evaluation_score(&v.metrics));
        entry.1.insert(trial_id);
        let cur = max_rung_by_trial.entry(trial_id).or_insert(0);
        if rung > *cur {
            *cur = rung;
        }
    }

    let last_rung = by_rung.keys().copied().max().unwrap_or(0);
    by_rung
        .into_iter()
        .map(|(rung, (scores, trials))| {
            let mean = if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f32>() / scores.len() as f32
            };
            let survivors = if rung < last_rung {
                Some(
                    max_rung_by_trial
                        .values()
                        .filter(|max_rung| **max_rung > rung)
                        .count(),
                )
            } else {
                None
            };
            RungRow {
                rung,
                trials: trials.len(),
                mean_composite: mean,
                survivors,
            }
        })
        .collect()
}

#[component]
fn RungSummaryPanel(rows: Vec<RungRow>) -> impl IntoView {
    view! {
        <div>
            <div class="muted text-xs mb-2">
                "Successive halving sliced trials into rungs; each rung scores survivors on a larger question subset."
            </div>
            <table class="variants-table">
                <thead>
                    <tr>
                        <th class="num">"Rung"</th>
                        <th class="num">"Trials"</th>
                        <th class="num">"Mean composite"</th>
                        <th class="num">"Survivors"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|r| view! {
                        <tr>
                            <td class="num">{r.rung}</td>
                            <td class="num">{r.trials}</td>
                            <td class="num">{format!("{:.1}%", r.mean_composite * 100.0)}</td>
                            <td class="num muted">{
                                r.survivors
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "—".into())
                            }</td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
