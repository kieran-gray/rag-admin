use std::cmp::Ordering;

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::server_functions::evaluation::get_run;
use crate::shared::contracts::{aggregate_type, EvaluationRunDto, RunKindDto};
use crate::shared::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, Status, StatusPill, Surface};
use crate::ui::pages::evaluate::state::EvaluateSelection;

#[component]
pub fn ResultsStep(selection: EvaluateSelection, on_back: Callback<()>) -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let run = Resource::new(
        move || (selection.run_id.get(), invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_run(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    Effect::new(move |_| {
        if !selection.just_launched.get() {
            return;
        }
        let Some(Ok(Some(r))) = run.get() else {
            return;
        };
        let in_flight = matches!(r.status.as_str(), "running" | "pending");
        if !in_flight {
            return;
        }
        let tab = match r.kind {
            RunKindDto::Optimization => "progress",
            RunKindDto::Manual => "variants",
        };
        let target = format!("/runs/{}?tab={tab}", r.run_id);
        selection.just_launched.set(false);
        use_navigate()(
            &target,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });

    view! {
        <div class="space-y-6">
            <Transition fallback=|| view! { <p class="muted text-sm">"Loading run…"</p> }>
                {move || run.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(None) => view! {
                        <Surface>
                            <EmptyState
                                title="No run selected"
                                body="Go back and pick a run, or launch a new one.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(Some(r)) => view! { <ResultsSummary run=r /> }.into_any(),
                })}
            </Transition>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Back"</span>
                    <span class="step-advance-eyebrow-label">"Change run"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-ghost"
                    on:click=move |_| on_back.run(())
                >
                    "← Back to optimize"
                </button>
            </div>
        </div>
    }
}

#[component]
fn ResultsSummary(run: EvaluationRunDto) -> impl IntoView {
    let run_id = run.run_id;
    let (status_kind, status_label) = run_status(&run.status);
    let short = run.run_id.to_string().chars().take(8).collect::<String>();
    let variant_count = run.variants.len();
    let leader = primary_leader(&run.variants).cloned();
    let leader_score = leader.as_ref().map(|v| evaluation_score(&v.metrics));
    let leader_label = leader.as_ref().map(|v| v.variant.label.clone());
    let created_at = run.created_at.clone();
    let default_tab = match run.kind {
        RunKindDto::Optimization => "progress",
        RunKindDto::Manual => "variants",
    };
    let open_href = format!("/runs/{run_id}?tab={default_tab}");

    view! {
        <Surface>
            <div class="flex items-start justify-between gap-4">
                <div>
                    <div class="eyebrow">"Run"</div>
                    <div class="text-lg font-mono mt-1">{format!("run-{short}")}</div>
                    <div class="text-xs muted mt-1">
                        {format!("{variant_count} variants · {created_at}")}
                    </div>
                </div>
                <StatusPill label=status_label.to_string() kind=status_kind />
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-px bg-[var(--color-border)] mt-4">
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Leader score"</div>
                    <div class="text-lg mt-1 font-mono text-[var(--color-accent)]">
                        {leader_score
                            .map(|s| format!("{:.1}%", s * 100.0))
                            .unwrap_or_else(|| "—".to_string())}
                    </div>
                    <div class="text-xs muted mt-1">"Weighted composite across metrics."</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Top variant"</div>
                    <div class="text-sm font-mono mt-1 truncate" title=leader_label.clone().unwrap_or_default()>
                        {leader_label.clone().unwrap_or_else(|| "—".to_string())}
                    </div>
                </div>
            </div>

            <div class="flex items-center justify-end gap-2 pt-4 mt-4 border-t border-[var(--color-border)]">
                <A
                    href=open_href
                    attr:class="btn btn-primary"
                >
                    "Open run →"
                </A>
            </div>
        </Surface>
    }
}

fn primary_leader(variants: &[EvaluationVariantResult]) -> Option<&EvaluationVariantResult> {
    leader_in_split(variants, EvaluationResultSplit::Holdout)
        .or_else(|| leader_in_split(variants, EvaluationResultSplit::Validation))
        .or_else(|| leader_in_split(variants, EvaluationResultSplit::Full))
        .or_else(|| leader_in_split(variants, EvaluationResultSplit::Tuning))
}

fn leader_in_split(
    variants: &[EvaluationVariantResult],
    split: EvaluationResultSplit,
) -> Option<&EvaluationVariantResult> {
    variants.iter().filter(|v| v.split == split).max_by(|a, b| {
        evaluation_score(&a.metrics)
            .partial_cmp(&evaluation_score(&b.metrics))
            .unwrap_or(Ordering::Equal)
    })
}

fn run_status(status: &str) -> (Status, &'static str) {
    match status {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        "pending" => (Status::Pending, "Pending"),
        _ => (Status::Neutral, "Unknown"),
    }
}
