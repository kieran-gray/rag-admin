use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_params_map, use_query_map};
use uuid::Uuid;

use crate::core::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::server_functions::evaluation::get_run;
use crate::ui::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};

#[component]
pub fn ReplicateComparePage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();

    let left_id = Memo::new(move |_| {
        params
            .with(|p| p.get("run_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });
    let right_id = Memo::new(move |_| {
        query
            .with(|q| q.get("with").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let runs = Resource::new(
        move || (left_id.get(), right_id.get()),
        move |(left, right)| async move {
            let l = match left {
                Some(id) => get_run(id).await.map_err(|e| e.to_string())?,
                None => None,
            };
            let r = match right {
                Some(id) => get_run(id).await.map_err(|e| e.to_string())?,
                None => None,
            };
            Ok::<_, String>((l, r))
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading runs…"</p> }>
            {move || runs.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok((Some(l), Some(r))) => view! {
                    <CompareView left=l right=r />
                }.into_any(),
                Ok(_) => view! {
                    <Surface>
                        <EmptyState
                            title="Missing run for comparison"
                            body="One of the runs in this comparison link wasn't found. Use the Replicate button on a finished optimization to land here with both runs populated.".to_string()
                        />
                    </Surface>
                }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn CompareView(
    left: crate::contracts::EvaluationRunDto,
    right: crate::contracts::EvaluationRunDto,
) -> impl IntoView {
    let left_id = left.run_id;
    let right_id = right.run_id;
    let left_short = left_id.to_string()[..8].to_string();
    let right_short = right_id.to_string()[..8].to_string();

    let left_champion = champion_of(&left.variants);
    let right_champion = champion_of(&right.variants);

    let same_config = match (left_champion.as_ref(), right_champion.as_ref()) {
        (Some(a), Some(b)) => a.variant.config == b.variant.config && a.options == b.options,
        _ => false,
    };
    let within_ci = match (left_champion.as_ref(), right_champion.as_ref()) {
        (Some(a), Some(b)) => ci_overlap(a, b),
        _ => false,
    };

    let verdict_kind = if same_config {
        Status::Ok
    } else if within_ci {
        Status::Info
    } else {
        Status::Fail
    };
    let verdict_label = if same_config {
        "Replication landed on the same config".to_string()
    } else if within_ci {
        "Different configs, but champions overlap within their 95% confidence intervals".to_string()
    } else {
        "Champions disagree beyond their 95% confidence intervals. The dataset may not be discriminating".to_string()
    };

    view! {
        <div>
            <PageHeader
                title=format!("replicate {left_short} vs {right_short}")
                eyebrow="Evaluations / Replication".to_string()
                subtitle="Side-by-side champion comparison between two optimization runs over the same dataset.".to_string()
                actions=Box::new(move || view! {
                    <StatusPill label=verdict_label.clone() kind=verdict_kind />
                }.into_any())
            />

            <div class="mb-4 flex items-center gap-3 flex-wrap">
                <A href="/evaluations" attr:class="muted text-sm">"← Back to evaluations"</A>
                <span class="muted">"·"</span>
                <A href=format!("/runs/{left_id}") attr:class="muted text-sm">{format!("View run {left_short}")}</A>
                <span class="muted">"·"</span>
                <A href=format!("/runs/{right_id}") attr:class="muted text-sm">{format!("View run {right_short}")}</A>
            </div>

            <Surface flush=true>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-px bg-[var(--color-border)]">
                    <ChampionCard
                        title=format!("Run {left_short}")
                        champion=left_champion
                    />
                    <ChampionCard
                        title=format!("Run {right_short}")
                        champion=right_champion
                    />
                </div>
            </Surface>
        </div>
    }
}

#[component]
fn ChampionCard(title: String, champion: Option<EvaluationVariantResult>) -> impl IntoView {
    let body = match champion {
        Some(c) => {
            let score = evaluation_score(&c.metrics);
            let half = c.metrics.composite_ci_half_width();
            let ci_label = if half > 0.0005 {
                format!("± {:.1}%", half * 100.0)
            } else {
                "—".into()
            };
            view! {
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">{title}</div>
                    <div class="text-lg mt-1 font-mono">{c.variant.label.clone()}</div>
                    <div class="text-2xl mt-2 font-mono text-[var(--color-accent)]">
                        {format!("{:.1}%", score * 100.0)}
                    </div>
                    <div class="text-xs muted mt-1">{format!("95% confidence interval {ci_label}")}</div>
                    <div class="mt-3 text-xs">
                        <div>{format!("top_k = {}", c.options.top_k)}</div>
                        <div>{format!("min_score = {:.2}", c.options.min_score())}</div>
                    </div>
                </div>
            }
            .into_any()
        }
        None => view! {
            <div class="bg-[var(--color-surface-1)] p-4">
                <div class="eyebrow">{title}</div>
                <div class="muted mt-2 text-sm">"No champion yet (run still in progress)."</div>
            </div>
        }
        .into_any(),
    };
    body
}

fn champion_of(variants: &[EvaluationVariantResult]) -> Option<EvaluationVariantResult> {
    let holdouts: Vec<&EvaluationVariantResult> = variants
        .iter()
        .filter(|v| v.split == EvaluationResultSplit::Holdout)
        .collect();
    if holdouts.is_empty() {
        return None;
    }
    holdouts
        .iter()
        .find(|v| v.selected)
        .or_else(|| {
            holdouts.iter().max_by(|a, b| {
                evaluation_score(&a.metrics)
                    .partial_cmp(&evaluation_score(&b.metrics))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|v| (*v).clone())
}

fn ci_overlap(a: &EvaluationVariantResult, b: &EvaluationVariantResult) -> bool {
    let a_lo = a.metrics.composite_ci_low;
    let a_hi = a.metrics.composite_ci_high;
    let b_lo = b.metrics.composite_ci_low;
    let b_hi = b.metrics.composite_ci_high;
    if a_hi <= a_lo || b_hi <= b_lo {
        let a_score = evaluation_score(&a.metrics);
        let b_score = evaluation_score(&b.metrics);
        return (a_score - b_score).abs() < 0.02;
    }
    a_hi.min(b_hi) >= a_lo.max(b_lo)
}
