use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{
    EmptyState, MetricBar, MetricKind, PageHeader, Status, StatusPill, Surface,
};
use crate::server_functions::evaluation::get_run;
use crate::shared::{aggregate_type, evaluation_score, EvaluationRunDto, EvaluationVariantResult};

#[component]
pub fn RunDetailPage() -> impl IntoView {
    let params = use_params_map();
    let run_id = Memo::new(move |_| {
        params
            .with(|p| p.get("run_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let run_invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let run = Resource::new(
        move || (run_id.get(), run_invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_run(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading run…"</p> }>
            {move || run.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Run not found"
                            body="This run id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(r)) => view! { <RunView run=r /> }.into_any(),
            })}
        </Transition>
    }
}

#[derive(Clone, Copy)]
struct MetricDef {
    name: &'static str,
    short: &'static str,
    help: &'static str,
}

const METRIC_DEFS: &[MetricDef] = &[
    MetricDef {
        name: "Recall",
        short: "R",
        help: "Fraction of each question's reference span that the retrieved chunks cover. \
               1.0 means every byte of every reference was returned. Penalised by missing content.",
    },
    MetricDef {
        name: "Precision",
        short: "P",
        help: "Fraction of the retrieved chunks' bytes that fall inside a reference span. \
               Penalised by retrieving extra non-relevant content alongside the answer.",
    },
    MetricDef {
        name: "IoU",
        short: "IoU",
        help: "Intersection-over-Union: overlap between retrieved and reference spans divided \
               by their union. A combined measure that punishes both missed and excess content.",
    },
    MetricDef {
        name: "Precision-ω",
        short: "Pω",
        help: "Precision over only the chunks whose spans touch a reference — \
               isolates retrieval quality from chunk-boundary noise.",
    },
];

#[component]
fn RunView(run: EvaluationRunDto) -> impl IntoView {
    let (status_kind, status_label) = match run.status.as_str() {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        _ => (Status::Neutral, "Unknown"),
    };
    let short = run.run_id.to_string()[..8].to_string();

    let mut variants = run.variants;
    variants.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let bests = best_per_metric(&variants);
    let leader_score = variants.first().map(|v| evaluation_score(&v.metrics));

    let variant_count = variants.len();
    let created_at = run.created_at.clone();

    view! {
        <div>
            <PageHeader
                title=format!("run-{short}")
                eyebrow="Evaluations / Run".to_string()
                subtitle=format!("{variant_count} variants · {created_at}")
                actions=Box::new(move || view! {
                    <StatusPill label=status_label.to_string() kind=status_kind />
                }.into_any())
            />

            <div class="mb-4">
                <A href="/evaluations" attr:class="muted text-sm">"← Back to evaluations"</A>
            </div>

            <RunSummary leader_score=leader_score variants_count=variant_count />

            <MetricLegend />

            {if variants.is_empty() {
                view! {
                    <Surface>
                        <EmptyState
                            title="No variants yet"
                            body="The run may still be in progress; variants land here as they're scored.".to_string()
                        />
                    </Surface>
                }.into_any()
            } else {
                view! {
                    <Surface title="Variants".to_string()>
                        <div class="space-y-5">
                            {variants.into_iter().enumerate().map(|(i, v)| view! {
                                <VariantCard
                                    variant=v
                                    leader=i == 0
                                    bests=bests
                                />
                            }).collect_view()}
                        </div>
                        <AxisLegend />
                    </Surface>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn RunSummary(leader_score: Option<f32>, variants_count: usize) -> impl IntoView {
    let weights = crate::shared::EvaluationScoreWeights::default();
    let score_str = leader_score
        .map(|s| format!("{:.1}%", s * 100.0))
        .unwrap_or_else(|| "—".to_string());
    view! {
        <Surface flush=true>
            <div class="grid grid-cols-1 md:grid-cols-3 gap-px bg-[var(--color-border)]">
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Variants compared"</div>
                    <div class="text-lg mt-1 font-mono">{variants_count}</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Leader score"</div>
                    <div class="text-lg mt-1 font-mono text-[var(--color-accent)]">{score_str}</div>
                    <div class="text-xs muted mt-1">"Weighted composite of all four metrics"</div>
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
fn MetricLegend() -> impl IntoView {
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
                        <span class="text-xs muted font-mono">"σ"</span>
                        <span class="muted">
                            "Standard deviation across the dataset's questions — a low σ means the variant scores consistently; a high σ means it does well on some questions and badly on others."
                        </span>
                    </div>
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                        <span class="font-medium">"Pink tick"</span>
                        <span class="text-xs muted font-mono">"▎"</span>
                        <span class="muted">
                            "Best score for that metric across all variants in the run. Lets you see how close a runner-up is to the leader on each dimension."
                        </span>
                    </div>
                </div>
            </details>
        </div>
    }
}

#[component]
fn VariantCard(
    variant: EvaluationVariantResult,
    leader: bool,
    bests: MetricBests,
) -> impl IntoView {
    let kind = if leader {
        MetricKind::Best
    } else {
        MetricKind::Default
    };
    let score = evaluation_score(&variant.metrics);
    let label = variant.variant.label.clone();
    let split = variant.split.as_str().to_string();
    let chunk_count = variant.chunk_count;
    let avg_tokens = variant.average_chunk_tokens;
    let selected = variant.selected;
    let m = variant.metrics;

    view! {
        <div class=move || format!(
            "surface-raised rounded p-4 {}",
            if leader { "border-l-2 border-l-[var(--color-accent)]" } else { "" }
        )>
            <div class="flex items-center justify-between mb-3 gap-3 flex-wrap">
                <div class="flex items-center gap-3 min-w-0">
                    {leader.then(|| view! { <span class="text-[var(--color-accent)]">"★"</span> })}
                    <span class="font-mono text-base truncate">{label}</span>
                    <span class="text-xs muted">{format!("{chunk_count} chunks · avg {avg_tokens} tok")}</span>
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-xs muted">{format!("split: {split}")}</span>
                    {selected.then(|| view! {
                        <span class="pill pill-ok">"selected"</span>
                    })}
                    <span class="font-mono text-sm">
                        <span class="muted text-xs mr-1">"score"</span>
                        {format!("{:.1}%", score * 100.0)}
                    </span>
                </div>
            </div>
            <div class="space-y-2">
                <MetricBar
                    label="Recall"
                    short="R"
                    help=METRIC_DEFS[0].help.to_string()
                    value=m.recall_mean
                    stddev=m.recall_std
                    best=bests.recall
                    kind=kind
                />
                <MetricBar
                    label="Precision"
                    short="P"
                    help=METRIC_DEFS[1].help.to_string()
                    value=m.precision_mean
                    stddev=m.precision_std
                    best=bests.precision
                    kind=kind
                />
                <MetricBar
                    label="IoU"
                    short="IoU"
                    help=METRIC_DEFS[2].help.to_string()
                    value=m.iou_mean
                    stddev=m.iou_std
                    best=bests.iou
                    kind=kind
                />
                <MetricBar
                    label="Precision-ω"
                    short="Pω"
                    help=METRIC_DEFS[3].help.to_string()
                    value=m.precision_omega_mean
                    stddev=m.precision_omega_std
                    best=bests.precision_omega
                    kind=kind
                />
            </div>
        </div>
    }
}

#[component]
fn AxisLegend() -> impl IntoView {
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

#[derive(Clone, Copy, Default)]
struct MetricBests {
    recall: f32,
    precision: f32,
    iou: f32,
    precision_omega: f32,
}

fn best_per_metric(variants: &[EvaluationVariantResult]) -> MetricBests {
    let mut b = MetricBests::default();
    for v in variants {
        b.recall = b.recall.max(v.metrics.recall_mean);
        b.precision = b.precision.max(v.metrics.precision_mean);
        b.iou = b.iou.max(v.metrics.iou_mean);
        b.precision_omega = b.precision_omega.max(v.metrics.precision_omega_mean);
    }
    b
}
