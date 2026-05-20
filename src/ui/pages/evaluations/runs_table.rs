use leptos::prelude::*;

use crate::shared::contracts::{BestVariantDto, RecentEvaluationRunDto, RunStatusFilterDto};
use crate::ui::components::primitives::{
    SkeletonColumn, SkeletonRows, Status, StatusPill, TitleCell,
};
use crate::ui::pages::shared::format_when;

pub fn runs_skeleton_columns() -> Vec<SkeletonColumn> {
    vec![
        SkeletonColumn::with_sub("70%", "40%"),
        SkeletonColumn::new("5rem"),
        SkeletonColumn::with_sub("80%", "60%"),
        SkeletonColumn::right("3rem"),
        SkeletonColumn::right("4rem"),
        SkeletonColumn::empty(),
    ]
}

#[component]
pub fn RunsTable(runs: Vec<RecentEvaluationRunDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-[34%]">"Document"</th>
                    <th>"Status"</th>
                    <th class="w-[34%]">"Top variant"</th>
                    <th class="text-right">"Score"</th>
                    <th class="text-right">"When"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                {runs.into_iter().map(|r| view! { <RunRow run=r /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
pub fn SkeletonRunsTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-[34%]">"Document"</th>
                    <th>"Status"</th>
                    <th class="w-[34%]">"Top variant"</th>
                    <th class="text-right">"Score"</th>
                    <th class="text-right">"When"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                <SkeletonRows columns=runs_skeleton_columns() />
            </tbody>
        </table>
    }
}

#[component]
fn RunRow(run: RecentEvaluationRunDto) -> impl IntoView {
    use leptos_router::components::A;

    let href = format!("/runs/{}", run.run_id);
    let title = run
        .document_title
        .clone()
        .unwrap_or_else(|| "Untitled document".to_string());
    let variant_count = run.variant_count;
    let variants_scored = run.variants_scored;
    let when = format_when(&run.created_at);
    let when_full = run.created_at.clone();
    let subtitle = variant_subtitle(&run.status, variants_scored, variant_count);

    view! {
        <tr>
            <td>
                <A href=href.clone() attr:class="block">
                    <TitleCell title=title sub=subtitle />
                </A>
            </td>
            <td>
                <RunStatusCell
                    status=run.status.clone()
                    variants_scored=variants_scored
                    variant_count=variant_count
                    failure_reason=run.failure_reason.clone()
                />
            </td>
            <td>
                {match run.best.as_ref() {
                    Some(best) => view! { <BestVariantCell best=best.clone() /> }.into_any(),
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right">
                {match run.best.as_ref() {
                    Some(best) => {
                        let score = best.score * 100.0;
                        view! {
                            <span class="font-mono text-text" style="font-size: 0.95rem">
                                {format!("{score:.1}%")}
                            </span>
                        }.into_any()
                    }
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right text-xs muted" title=when_full>{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

#[component]
fn RunStatusCell(
    status: String,
    variants_scored: u32,
    variant_count: u32,
    failure_reason: Option<String>,
) -> impl IntoView {
    let (label, kind) = run_status_label(&status);
    let suffix = match status.as_str() {
        "running" => Some(format!("{variants_scored}/{variant_count}")),
        "pending" => (variant_count > 0).then(|| format!("0/{variant_count}")),
        _ => None,
    };
    let combined = match suffix {
        Some(s) => format!("{label} {s}"),
        None => label,
    };
    let title_attr = failure_reason.unwrap_or_default();
    view! {
        <span title=title_attr>
            <StatusPill label=combined kind=kind />
        </span>
    }
}

#[component]
fn BestVariantCell(best: BestVariantDto) -> impl IntoView {
    let label = best.label.clone();
    let metrics = best.metrics.clone();
    let top_k = best.options.top_k;
    let min_score = best.options.min_score();

    view! {
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2 min-w-0">
                <span class="font-mono text-xs truncate" title=label.clone()>{label.clone()}</span>
                <span class="faint text-xs whitespace-nowrap">
                    {format!("k={top_k} · min {min_score:.2}")}
                </span>
            </div>
            <div class="faint text-xs font-mono whitespace-nowrap">
                {format!(
                    "R {:.0} · P {:.0} · IoU {:.0}",
                    metrics.recall_mean * 100.0,
                    metrics.precision_mean * 100.0,
                    metrics.iou_mean * 100.0,
                )}
            </div>
        </div>
    }
}

pub fn run_status_label(status: &str) -> (String, Status) {
    match status {
        "completed" => ("Completed".to_string(), Status::Ok),
        "failed" => ("Failed".to_string(), Status::Fail),
        "running" => ("Running".to_string(), Status::Pending),
        "pending" => ("Pending".to_string(), Status::Pending),
        _ => ("Unknown".to_string(), Status::Neutral),
    }
}

pub fn status_color(status: RunStatusFilterDto) -> &'static str {
    match status {
        RunStatusFilterDto::Completed => "var(--status-ok)",
        RunStatusFilterDto::Running | RunStatusFilterDto::Pending => "var(--status-pending)",
        RunStatusFilterDto::Failed => "var(--status-fail)",
    }
}

fn variant_subtitle(status: &str, scored: u32, total: u32) -> String {
    if status == "running" || status == "pending" {
        if total == 0 {
            "preparing variants".to_string()
        } else {
            format!("{scored} of {total} variants scored")
        }
    } else if total == 1 {
        "1 variant".to_string()
    } else {
        format!("{total} variants")
    }
}
