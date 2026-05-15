use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};
use crate::server_functions::evaluation::get_run;
use crate::shared::{aggregate_type, evaluation_score, EvaluationRunDto, EvaluationVariantResult};

#[component]
pub fn OptimizeProgressPage() -> impl IntoView {
    let params = use_params_map();
    let run_id = Memo::new(move |_| {
        params
            .with(|p| p.get("run_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let run = Resource::new(
        move || (run_id.get(), invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_run(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading optimization…"</p> }>
            {move || run.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Run not found"
                            body="This optimization run id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(r)) => view! { <ProgressView run=r /> }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn ProgressView(run: EvaluationRunDto) -> impl IntoView {
    let short = run.run_id.to_string()[..8].to_string();
    let status_kind = match run.status.as_str() {
        "completed" => Status::Ok,
        "failed" => Status::Fail,
        "running" => Status::Pending,
        _ => Status::Neutral,
    };
    let trial_count = run.variants.len();
    let best_so_far = best_so_far_score(&run.variants);
    let created_at = run.created_at.clone();
    let run_id = run.run_id.to_string();
    let status_label = run.status.clone();
    let status_label_header = status_label.clone();

    view! {
        <div>
            <PageHeader
                title=format!("optimize-{short}")
                eyebrow="Evaluations / Optimization".to_string()
                subtitle=format!("{trial_count} trials · {created_at}")
                actions=Box::new(move || view! {
                    <StatusPill label=status_label_header.clone() kind=status_kind />
                }.into_any())
            />

            <div class="mb-4 flex items-center gap-3">
                <A href="/evaluations" attr:class="muted text-sm">"← Back to evaluations"</A>
                <span class="muted">"·"</span>
                <A href=format!("/runs/{run_id}") attr:class="muted text-sm">"Full run detail"</A>
            </div>

            <Surface flush=true>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-px bg-[var(--color-border)]">
                    <div class="bg-[var(--color-surface-1)] p-4">
                        <div class="eyebrow">"Trials evaluated"</div>
                        <div class="text-lg mt-1 font-mono">{trial_count}</div>
                    </div>
                    <div class="bg-[var(--color-surface-1)] p-4">
                        <div class="eyebrow">"Best so far · tuning"</div>
                        <div class="text-lg mt-1 font-mono text-[var(--color-accent)]">
                            {best_so_far
                                .map(|s| format!("{:.1}%", s * 100.0))
                                .unwrap_or_else(|| "—".into())}
                        </div>
                        <div class="text-xs muted mt-1">
                            "Preliminary, on the optimizer's tuning split. Not the deployment number."
                        </div>
                    </div>
                    <div class="bg-[var(--color-surface-1)] p-4">
                        <div class="eyebrow">"Status"</div>
                        <div class="text-lg mt-1 font-mono">{status_label}</div>
                    </div>
                </div>
            </Surface>

            {{
                let variants_for_chart = run.variants.clone();
                let variants_for_rungs = run.variants.clone();
                let variants_for_retired = run.variants.clone();
                let variants_for_table = run.variants;
                view! {
                    {(!variants_for_chart.is_empty()).then(move || view! {
                        <Surface title="Best-so-far".to_string()>
                            <BestSoFarChart trials=variants_for_chart />
                        </Surface>
                    })}

                    {{
                        let rungs = rung_summary(&variants_for_rungs);
                        (!rungs.is_empty()).then(|| view! { <RungSummaryPanel rows=rungs /> })
                    }}

                    {{
                        let retired = retired_trials(&variants_for_retired);
                        (!retired.is_empty()).then(|| view! { <RetiredBranchesPanel rows=retired /> })
                    }}

                    <Surface title="Trials".to_string()>
                        {if variants_for_table.is_empty() {
                            view! {
                                <EmptyState
                                    title="No trials yet"
                                    body="The optimizer is warming up; trials will appear here as they're scored.".to_string()
                                />
                            }.into_any()
                        } else {
                            view! { <TrialList trials=variants_for_table /> }.into_any()
                        }}
                    </Surface>
                }
            }}
        </div>
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
            band_path.push_str(&format!("{cmd} {:.1} {:.1} ", to_x(i), to_y(*hi)));
        }
        for (i, (_, lo, _)) in best.iter().enumerate().rev() {
            band_path.push_str(&format!("L {:.1} {:.1} ", to_x(i), to_y(*lo)));
        }
        band_path.push('Z');
    }

    let svg_view_box = format!("0 0 {W} {H}");
    let final_score = best.last().map(|(s, _, _)| *s).unwrap_or(0.0);

    view! {
        <div class="text-xs muted mb-2">
            "Running best composite across trials, with the leader's 95% confidence interval as a band. The plateau tells you when the optimizer stopped finding improvements."
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
    }.into_any()
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
    use std::collections::BTreeMap;
    let mut by_rung: BTreeMap<u32, (Vec<f32>, std::collections::HashSet<u32>)> = BTreeMap::new();
    let mut max_rung_by_trial: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::new();

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

#[derive(Clone)]
struct RetiredRow {
    trial_id: u32,
    last_composite: f32,
    retired_at_rung: u32,
    variant_label: String,
}

fn retired_trials(variants: &[EvaluationVariantResult]) -> Vec<RetiredRow> {
    use std::collections::HashMap;
    let mut max_rung_by_trial: HashMap<u32, (u32, f32, String)> = HashMap::new();
    let mut overall_max_rung = 0u32;
    for v in variants {
        let Some((trial_id, rung)) = parse_trial_rung(&v.variant.label) else {
            continue;
        };
        if rung > overall_max_rung {
            overall_max_rung = rung;
        }
        let entry = max_rung_by_trial
            .entry(trial_id)
            .or_insert((0, 0.0, v.variant.label.clone()));
        if rung > entry.0 {
            *entry = (rung, evaluation_score(&v.metrics), v.variant.label.clone());
        }
    }
    let mut out: Vec<RetiredRow> = max_rung_by_trial
        .into_iter()
        .filter(|(_, (rung, _, _))| *rung < overall_max_rung)
        .map(|(trial_id, (rung, composite, label))| RetiredRow {
            trial_id,
            last_composite: composite,
            retired_at_rung: rung,
            variant_label: label,
        })
        .collect();
    out.sort_by(|a, b| {
        b.last_composite
            .partial_cmp(&a.last_composite)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[component]
fn RungSummaryPanel(rows: Vec<RungRow>) -> impl IntoView {
    view! {
        <Surface title="Rung summary".to_string()>
            <div class="text-xs muted mb-3">
                "Successive halving sliced your trials into rungs; each rung scores survivors on a larger question subset. Drop-offs reflect point-estimate ranking. They aren't a variance-aware test, so weak-looking configs may still be within noise at small question counts."
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
        </Surface>
    }
}

#[component]
fn RetiredBranchesPanel(rows: Vec<RetiredRow>) -> impl IntoView {
    let count = rows.len();
    view! {
        <Surface title=format!("Retired branches · {count} trials") >
            <div class="text-xs muted mb-3">
                "Trials that didn't survive their rung. Look here if you suspect the optimizer pruned an interesting config too early."
            </div>
            <table class="variants-table">
                <thead>
                    <tr>
                        <th class="num">"Trial"</th>
                        <th>"Label"</th>
                        <th class="num">"Last composite"</th>
                        <th class="num">"Retired at rung"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|r| view! {
                        <tr>
                            <td class="num">{r.trial_id}</td>
                            <td><span class="font-mono">{r.variant_label}</span></td>
                            <td class="num">{format!("{:.1}%", r.last_composite * 100.0)}</td>
                            <td class="num muted">{r.retired_at_rung}</td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </Surface>
    }
}

fn best_so_far_score(variants: &[EvaluationVariantResult]) -> Option<f32> {
    variants
        .iter()
        .map(|v| evaluation_score(&v.metrics))
        .fold(None, |acc, s| match acc {
            None => Some(s),
            Some(prev) => Some(prev.max(s)),
        })
}

#[component]
fn TrialList(trials: Vec<EvaluationVariantResult>) -> impl IntoView {
    let mut trials = trials;
    trials.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    view! {
        <table class="variants-table">
            <thead>
                <tr>
                    <th class="num">"#"</th>
                    <th>"Trial"</th>
                    <th class="num">"TopK"</th>
                    <th class="num">"Min score"</th>
                    <th class="num">"Score"</th>
                    <th class="num" title="95% bootstrap confidence interval half-width on the composite score">"95% confidence interval"</th>
                </tr>
            </thead>
            <tbody>
                {trials.into_iter().enumerate().map(|(i, v)| {
                    let score = evaluation_score(&v.metrics);
                    let half = v.metrics.composite_ci_half_width();
                    let ci_label = if half > 0.0005 {
                        format!("± {:.1}", half * 100.0)
                    } else {
                        "—".into()
                    };
                    view! {
                        <tr>
                            <td class="num muted">{i + 1}</td>
                            <td><span class="font-mono">{v.variant.label.clone()}</span></td>
                            <td class="num">{v.options.top_k}</td>
                            <td class="num">{format!("{:.2}", v.options.min_score())}</td>
                            <td class="num"><strong>{format!("{:.1}%", score * 100.0)}</strong></td>
                            <td class="num muted">{ci_label}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}
