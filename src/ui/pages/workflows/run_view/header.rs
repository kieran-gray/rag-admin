use leptos::prelude::*;
use leptos_router::components::A;

use crate::shared::contracts::{EvaluationRunDto, RunKindDto};
use crate::shared::OptimizationBudget;
use crate::ui::components::primitives::{PageHeader, Status, StatusPill};
use crate::ui::pages::shared::format_when;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn RunHeader(run: EvaluationRunDto, compare_with: Option<uuid::Uuid>) -> impl IntoView {
    let short = run.run_id.to_string().chars().take(8).collect::<String>();
    let title = format!("run-{short}");
    let run_id = run.run_id;
    let (status_kind, status_label) =
        run_status(&run.status, run.variants_scored, run.variant_count);

    let kind_chip = kind_chip_label(&run);
    let context_line = context_line(&run);

    let dataset_href = format!("/artifacts/datasets/{}", run.dataset_id);
    let workflow_href = run
        .document_id
        .map(|id| format!("/evaluate/by-id/{id}?dataset={}&step=run", run.dataset_id));

    let header_actions = {
        let kind_chip = kind_chip.clone();
        Box::new(move || {
            view! {
                <div class="flex items-center gap-2 flex-wrap">
                    <KindBadge label=kind_chip.clone() />
                    <StatusPill label=status_label.clone() kind=status_kind />
                </div>
            }
            .into_any()
        })
    };

    let compare_link = compare_with.map(|other| {
        let in_compare = format!("/workflows/runs/{run_id}?with={other}");
        let leave_compare = format!("/workflows/runs/{run_id}");
        (in_compare, leave_compare, other)
    });

    view! {
        <PageHeader
            title=title
            eyebrow="Evaluations / Run".to_string()
            subtitle=context_line
            actions=header_actions
        />

        <div class="mb-4 flex items-center gap-3 flex-wrap text-sm">
            {workflow_href.map(|href| view! {
                <A href=href attr:class="muted hover:text-text">"← Back to workflow"</A>
            })}
            <A href=dataset_href attr:class="muted hover:text-text">"Open dataset"</A>
            <A href="/workflows/runs" attr:class="muted hover:text-text">"All evaluations"</A>
            {compare_link.map(|(_in_compare, leave, other)| {
                let other_short = other.to_string().chars().take(8).collect::<String>();
                view! {
                    <span class="muted">"·"</span>
                    <span class="pill pill-accent text-xs">{format!("Comparing with {other_short}")}</span>
                    <A href=leave attr:class="muted hover:text-text">"Exit comparison"</A>
                }
            })}
        </div>
    }
}

#[component]
fn KindBadge(label: String) -> impl IntoView {
    view! {
        <span class="pill bg-[var(--color-accent-soft)] text-[var(--color-accent)] whitespace-nowrap">
            {label}
        </span>
    }
}

fn kind_chip_label(run: &EvaluationRunDto) -> String {
    match run.kind {
        RunKindDto::Optimization => {
            let budget = run
                .optimization
                .as_ref()
                .map(|o| budget_label(o.budget))
                .unwrap_or("");
            if budget.is_empty() {
                "Optimization".to_string()
            } else {
                format!("Optimization · {budget}")
            }
        }
        RunKindDto::Manual => format!("Manual · {} variants", run.variant_count.max(1)),
    }
}

fn context_line(run: &EvaluationRunDto) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = run.document_title.as_ref().filter(|t| !t.is_empty()) {
        parts.push(format!("Document · {title}"));
    }
    let dataset_short = run
        .dataset_id
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();
    parts.push(format!("Dataset · {dataset_short}"));
    parts.push(format!("Started · {}", format_when(&run.created_at)));
    parts.join("  ·  ")
}

fn budget_label(budget: OptimizationBudget) -> &'static str {
    match budget {
        OptimizationBudget::Quick => "Quick",
        OptimizationBudget::Thorough => "Thorough",
        OptimizationBudget::Exhaustive => "Exhaustive",
    }
}

fn run_status(status: &str, scored: u32, total: u32) -> (Status, String) {
    match status {
        "completed" => (Status::Ok, "Completed".to_string()),
        "failed" => (Status::Fail, "Failed".to_string()),
        "running" => {
            if total > 0 {
                (Status::Pending, format!("Running {scored}/{total}"))
            } else {
                (Status::Pending, "Running".to_string())
            }
        }
        "pending" => (Status::Pending, "Pending".to_string()),
        "cancelled" => (Status::Cancel, "Cancelled".to_string()),
        _ => (Status::Neutral, "Unknown".to_string()),
    }
}
