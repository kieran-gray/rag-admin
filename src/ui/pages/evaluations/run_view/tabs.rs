use leptos::prelude::*;
use leptos_router::components::A;
use uuid::Uuid;

use crate::shared::contracts::EvaluationRunDto;
use crate::shared::contracts::RunKindDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTab {
    Progress,
    Variants,
    Compare,
}

impl RunTab {
    pub fn slug(self) -> &'static str {
        match self {
            RunTab::Progress => "progress",
            RunTab::Variants => "variants",
            RunTab::Compare => "compare",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RunTab::Progress => "Progress",
            RunTab::Variants => "Variants & diagnostics",
            RunTab::Compare => "Compare",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "progress" => Some(RunTab::Progress),
            "variants" => Some(RunTab::Variants),
            "compare" => Some(RunTab::Compare),
            _ => None,
        }
    }
}

pub fn default_tab_for(run: &EvaluationRunDto) -> RunTab {
    let in_flight = matches!(run.status.as_str(), "running" | "pending");
    match (run.kind, in_flight) {
        (RunKindDto::Optimization, true) => RunTab::Progress,
        _ => RunTab::Variants,
    }
}

pub fn progress_tab_available(run: &EvaluationRunDto) -> bool {
    if run.kind == RunKindDto::Optimization {
        return true;
    }
    run.variants
        .iter()
        .any(|v| v.variant.label.starts_with("trial-"))
}

#[component]
pub fn RunTabs(
    run_id: Uuid,
    active: RunTab,
    progress_available: bool,
    compare_available: bool,
) -> impl IntoView {
    view! {
        <div class="surface flex p-0 overflow-hidden sticky top-0 z-10">
            {progress_available.then(|| view! {
                <TabLink run_id=run_id tab=RunTab::Progress active=active />
            })}
            <TabLink run_id=run_id tab=RunTab::Variants active=active />
            <TabLink run_id=run_id tab=RunTab::Compare active=active disabled=!compare_available />
        </div>
    }
}

#[component]
fn TabLink(
    run_id: Uuid,
    tab: RunTab,
    active: RunTab,
    #[prop(optional)] disabled: bool,
) -> impl IntoView {
    let href = format!("/runs/{run_id}?tab={}", tab.slug());
    let is_active = tab == active;
    let class = if is_active {
        "flex-1 px-4 py-3 text-left text-sm font-semibold bg-[var(--color-surface-2)] text-[var(--color-text)]"
    } else if disabled {
        "flex-1 px-4 py-3 text-left text-sm muted opacity-50 cursor-not-allowed"
    } else {
        "flex-1 px-4 py-3 text-left text-sm muted hover:bg-[var(--color-surface-2)] hover:text-text"
    };
    let label = tab.label();

    if disabled {
        view! {
            <span class=class title="Replicate this run to compare side-by-side">{label}</span>
        }
        .into_any()
    } else {
        view! {
            <A href=href attr:class=class>{label}</A>
        }
        .into_any()
    }
}
