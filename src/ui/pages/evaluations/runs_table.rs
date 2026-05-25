use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::shared::contracts::{
    BestVariantDto, RecentEvaluationRunDto, RunKindDto, RunStatusFilterDto,
};
use crate::shared::OptimizationBudget;
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
fn RunsTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[36%]">"Document"</th>
                <th class="w-[12%]">"Status"</th>
                <th class="w-[28%]">"Top variant"</th>
                <th class="w-[8%] text-right">"Score"</th>
                <th class="w-[12%] text-right">"When"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
pub fn RunsTable(runs: Vec<RecentEvaluationRunDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <RunsTableHead />
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
            <RunsTableHead />
            <tbody>
                <SkeletonRows columns=runs_skeleton_columns() />
            </tbody>
        </table>
    }
}

#[component]
fn RunRow(run: RecentEvaluationRunDto) -> impl IntoView {
    use leptos_router::components::A;

    let href = format!("/evaluations/runs/{}", run.run_id);
    let title = run
        .document_title
        .clone()
        .unwrap_or_else(|| "Untitled document".to_string());
    let variant_count = run.variant_count;
    let variants_scored = run.variants_scored;
    let when = format_when(&run.created_at);
    let when_full = run.created_at.clone();
    let subtitle = variant_subtitle(&run.status, variants_scored, variant_count);

    let nav_href = href.clone();
    let on_row_click = move |ev: leptos::ev::MouseEvent| {
        if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
            return;
        }
        use_navigate()(&nav_href, NavigateOptions::default());
    };

    view! {
        <tr on:click=on_row_click>
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
                <TopVariantCell run=run.clone() />
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
            <td class="text-right text-xs muted whitespace-nowrap" title=when_full>{when}</td>
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
fn TopVariantCell(run: RecentEvaluationRunDto) -> impl IntoView {
    let kind = run.kind;
    let kind_line = kind_summary(&run);

    view! {
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2 min-w-0">
                <KindChip kind=kind />
                <span class="text-xs muted truncate">{kind_line}</span>
            </div>
            {match run.best.as_ref() {
                Some(best) => view! { <BestVariantSubline best=best.clone() /> }.into_any(),
                None => view! { <div class="faint text-xs">"—"</div> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn KindChip(kind: RunKindDto) -> impl IntoView {
    let class = match kind {
        RunKindDto::Optimization => {
            "pill text-xs whitespace-nowrap bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
        }
        RunKindDto::Manual => "pill text-xs whitespace-nowrap pill-neutral",
    };
    view! {
        <span class=class>{kind.label()}</span>
    }
}

#[component]
fn BestVariantSubline(best: BestVariantDto) -> impl IntoView {
    let label = best.label.clone();
    let top_k = best.options.top_k;
    let min_score = best.options.min_score();

    let title_attr = label.clone();
    view! {
        <div class="flex items-center gap-2 min-w-0">
            <span class="font-mono text-xs truncate" title=title_attr>{label}</span>
            <span class="faint text-xs whitespace-nowrap">
                {format!("k={top_k} · min {min_score:.2}")}
            </span>
        </div>
    }
}

fn kind_summary(run: &RecentEvaluationRunDto) -> String {
    match run.kind {
        RunKindDto::Optimization => {
            let budget = run
                .optimization
                .as_ref()
                .map(|o| budget_label(o.budget))
                .unwrap_or("");
            let in_flight = matches!(run.status.as_str(), "running" | "pending");
            if in_flight {
                if budget.is_empty() {
                    format!("{} trials so far", run.variants_scored)
                } else {
                    format!("{budget} · {} trials so far", run.variants_scored)
                }
            } else if budget.is_empty() {
                String::new()
            } else {
                budget.to_string()
            }
        }
        RunKindDto::Manual => {
            let count = run.variant_count;
            if count == 1 {
                "1 variant".to_string()
            } else {
                format!("{count} variants")
            }
        }
    }
}

fn budget_label(budget: OptimizationBudget) -> &'static str {
    match budget {
        OptimizationBudget::Quick => "Quick",
        OptimizationBudget::Thorough => "Thorough",
        OptimizationBudget::Exhaustive => "Exhaustive",
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
