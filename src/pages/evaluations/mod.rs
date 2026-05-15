use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};
use crate::server_functions::evaluation::get_recent_runs;
use crate::shared::{aggregate_type, BestVariantDto, RecentEvaluationRunDto};

mod dataset_detail;
mod optimize_progress;
mod replicate_compare;
mod run_detail;

pub use dataset_detail::DatasetDetailPage;
pub use optimize_progress::OptimizeProgressPage;
pub use replicate_compare::ReplicateComparePage;
pub use run_detail::RunDetailPage;

const RECENT_LIMIT: u32 = 25;

#[component]
pub fn EvaluationsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let runs = Resource::new(
        move || invalidator.get(),
        |_| async move { get_recent_runs(RECENT_LIMIT).await },
    );

    view! {
        <div>
            <PageHeader
                title="Evaluations"
                subtitle="Recent evaluation runs and their top-scoring chunking configuration."
                    .to_string()
            />

            <Suspense fallback=|| view! {
                <Surface flush=true>
                    <div class="p-6 muted text-sm">"Loading runs…"</div>
                </Surface>
            }>
                {move || runs.get().map(|res| match res {
                    Ok(list) if list.is_empty() => view! {
                        <Surface>
                            <EmptyState
                                title="No evaluation runs yet"
                                body="Open a document, generate a dataset, and launch a run — results land here as they complete.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(list) => view! { <RunsTable runs=list /> }.into_any(),
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn RunsTable(runs: Vec<RecentEvaluationRunDto>) -> impl IntoView {
    let total = runs.len();
    let completed = runs.iter().filter(|r| r.status == "completed").count();
    let running = runs
        .iter()
        .filter(|r| r.status == "running" || r.status == "pending")
        .count();
    let failed = runs.iter().filter(|r| r.status == "failed").count();

    view! {
        <Surface flush=true>
            <table class="data-table">
                <thead>
                    <tr>
                        <th class="w-[28%]">"Document"</th>
                        <th>"Run"</th>
                        <th>"Status"</th>
                        <th>"Top variant"</th>
                        <th class="text-right">"Score"</th>
                        <th class="text-right">"When"</th>
                        <th class="w-8 text-right"></th>
                    </tr>
                </thead>
                <tbody>
                    {runs.into_iter().map(|r| view! { <RunRow run=r /> }).collect_view()}
                </tbody>
            </table>
            <div class="px-4 py-2.5 border-t border-[var(--color-border)] flex items-center gap-4 text-xs muted">
                <span>{format!("{total} runs")}</span>
                <span class="faint">"·"</span>
                <span>{format!("{completed} completed")}</span>
                {(running > 0).then(|| view! {
                    <>
                        <span class="faint">"·"</span>
                        <span style="color: var(--status-pending)">
                            {format!("{running} in progress")}
                        </span>
                    </>
                })}
                {(failed > 0).then(|| view! {
                    <>
                        <span class="faint">"·"</span>
                        <span style="color: var(--status-fail)">
                            {format!("{failed} failed")}
                        </span>
                    </>
                })}
            </div>
        </Surface>
    }
}

#[component]
fn RunRow(run: RecentEvaluationRunDto) -> impl IntoView {
    let href = format!("/runs/{}", run.run_id);
    let (status_label, status_kind) = run_status(&run.status);
    let run_short = run.run_id.to_string()[..8].to_string();
    let when = run
        .created_at
        .get(..16)
        .unwrap_or(&run.created_at)
        .to_string();
    let title = run
        .document_title
        .clone()
        .unwrap_or_else(|| "Unknown document".to_string());
    let title_attr = title.clone();
    let document_short = run.document_id.to_string()[..8].to_string();
    let variant_count = run.variant_count;

    view! {
        <tr>
            <td>
                <A href=href.clone() attr:class="block" attr:title=title_attr>
                    <div class="text-text font-medium line-clamp-2 break-words">
                        {title}
                    </div>
                    <div class="faint text-xs mt-0.5 font-mono">
                        {format!("doc {document_short}")}
                    </div>
                </A>
            </td>
            <td>
                <A href=href.clone() attr:class="block">
                    <span class="font-mono text-xs">{format!("run-{run_short}")}</span>
                    <div class="faint text-xs mt-0.5">{format!("{variant_count} variants")}</div>
                </A>
            </td>
            <td><StatusPill label=status_label kind=status_kind /></td>
            <td>
                {match run.best.clone() {
                    Some(best) => view! { <BestVariantCell best /> }.into_any(),
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right">
                {match run.best.as_ref() {
                    Some(best) => view! {
                        <span class="font-mono text-text">
                            {format!("{:.1}%", best.score * 100.0)}
                        </span>
                    }.into_any(),
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right text-xs muted font-mono">{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

#[component]
fn BestVariantCell(best: BestVariantDto) -> impl IntoView {
    let config_label = best.config.describe();
    let top_k = best.options.top_k;
    let min_score = best.options.min_score();
    let label = best.label.clone();
    let metrics = best.metrics.clone();

    view! {
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2">
                <span class="font-mono text-xs truncate" title=label.clone()>{label.clone()}</span>
                <span class="faint text-xs whitespace-nowrap">
                    {format!("topK {top_k} · min {min_score:.2}")}
                </span>
            </div>
            <div class="faint text-xs truncate" title=config_label.clone()>
                {config_label.clone()}
            </div>
            <div class="faint text-xs font-mono whitespace-nowrap">
                {format!(
                    "R {:.0} · P {:.0} · IoU {:.0} · Pω {:.0}",
                    metrics.recall_mean * 100.0,
                    metrics.precision_mean * 100.0,
                    metrics.iou_mean * 100.0,
                    metrics.precision_omega_mean * 100.0,
                )}
            </div>
        </div>
    }
}

fn run_status(status: &str) -> (String, Status) {
    match status {
        "completed" => ("Completed".to_string(), Status::Ok),
        "failed" => ("Failed".to_string(), Status::Fail),
        "running" => ("Running".to_string(), Status::Pending),
        "pending" => ("Pending".to_string(), Status::Pending),
        _ => ("Unknown".to_string(), Status::Neutral),
    }
}
