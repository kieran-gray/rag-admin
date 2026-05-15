use leptos::prelude::*;
use uuid::Uuid;

use crate::components::primitives::{Status, StatusPill, Surface};
use crate::server_functions::evaluation::start_run_optimization;
use crate::shared::{
    OptimizationBudget, OptimizationConfig, OptimizationScope, RunOptimizationRequestDto,
};

fn describe_budget(b: OptimizationBudget) -> String {
    b.describe().to_string()
}

#[component]
pub fn OptimizeLauncher(
    document_id: Uuid,
    #[prop(optional)] dataset_id: Option<Uuid>,
    #[prop(optional)] pipeline_configuration_id: Option<Uuid>,
) -> impl IntoView {
    let _ = document_id;
    let (budget, set_budget) = signal(OptimizationBudget::Thorough);
    let (scope, set_scope) = signal(OptimizationScope::Both);
    let (judges, set_judges) = signal(false);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let on_launch = move |_| {
        let Some(ds) = dataset_id else {
            set_error.set(Some(
                "Pick a dataset before launching an optimization.".into(),
            ));
            return;
        };
        let Some(pipeline) = pipeline_configuration_id else {
            set_error.set(Some(
                "Pick a pipeline before launching an optimization.".into(),
            ));
            return;
        };
        let request = RunOptimizationRequestDto {
            dataset_id: ds,
            pipeline_configuration_id: pipeline,
            optimization: OptimizationConfig {
                budget: budget.get_untracked(),
                scope: scope.get_untracked(),
                judges_enabled: judges.get_untracked(),
                seed: None,
            },
        };
        set_busy.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match start_run_optimization(request).await {
                Ok(info) => {
                    let run_id = info.job_id;
                    let _ = leptos::web_sys::window().and_then(|w| {
                        w.location()
                            .set_href(&format!("/runs/{run_id}/optimize"))
                            .ok()
                    });
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    };

    view! {
        <Surface title="Find best configuration".to_string()>
            <p class="text-sm muted mb-4">
                "Programmatic search over chunking and retrieval parameters. The optimizer proposes trials, retires losers via successive halving, and validates the champion on a held-out question set."
            </p>

            <div class="grid grid-cols-1 md:grid-cols-3 gap-3 mb-4">
                <BudgetCard
                    title="Quick"
                    description=describe_budget(OptimizationBudget::Quick)
                    active=move || budget.get() == OptimizationBudget::Quick
                    on_select=move |_| set_budget.set(OptimizationBudget::Quick)
                />
                <BudgetCard
                    title="Thorough"
                    description=describe_budget(OptimizationBudget::Thorough)
                    active=move || budget.get() == OptimizationBudget::Thorough
                    on_select=move |_| set_budget.set(OptimizationBudget::Thorough)
                />
                <BudgetCard
                    title="Exhaustive"
                    description=describe_budget(OptimizationBudget::Exhaustive)
                    active=move || budget.get() == OptimizationBudget::Exhaustive
                    on_select=move |_| set_budget.set(OptimizationBudget::Exhaustive)
                />
            </div>

            <div class="flex items-center gap-4 mb-3">
                <div class="eyebrow">"Scope"</div>
                <ScopeButton
                    label="Chunking"
                    active=move || scope.get() == OptimizationScope::Chunking
                    on_click=move |_| set_scope.set(OptimizationScope::Chunking)
                />
                <ScopeButton
                    label="Retrieval"
                    active=move || scope.get() == OptimizationScope::Retrieval
                    on_click=move |_| set_scope.set(OptimizationScope::Retrieval)
                />
                <ScopeButton
                    label="Both"
                    active=move || scope.get() == OptimizationScope::Both
                    on_click=move |_| set_scope.set(OptimizationScope::Both)
                />
            </div>

            <div class="flex items-center justify-between items-end gap-4 mt-4 mb-2">
                <label class="flex items-center gap-2 text-sm muted">
                    <input
                        type="checkbox"
                        prop:checked=move || judges.get()
                        on:change=move |ev| set_judges.set(event_target_checked(&ev))
                    />
                    "Enable LLM-judge pass on final rung (slow; extra cost)"
                </label>

                {move || error.get().map(|e| view! {
                    <StatusPill label=format!("Error: {e}") kind=Status::Fail />
                })}

                <button
                    type="button"
                    class="btn btn-primary"
                    prop:disabled=move || busy.get()
                    on:click=on_launch
                >
                    {move || if busy.get() { "Launching…" } else { "Find best configuration" }}
                </button>
            </div>
        </Surface>
    }
}

#[component]
fn BudgetCard(
    title: &'static str,
    description: String,
    active: impl Fn() -> bool + Send + Sync + 'static,
    on_select: impl Fn(leptos::ev::MouseEvent) + Send + Sync + 'static,
) -> impl IntoView {
    let active = StoredValue::new(active);
    let on_select = StoredValue::new(on_select);
    view! {
        <button
            type="button"
            class=move || format!(
                "surface-raised rounded p-3 text-left transition-colors {}",
                if active.with_value(|a| a()) {
                    "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                } else {
                    "hover:border-[var(--color-border-strong)]"
                }
            )
            on:click=move |ev| on_select.with_value(|f| f(ev))
        >
            <div class=move || format!(
                "font-medium {}",
                if active.with_value(|a| a()) { "text-[var(--color-accent)]" } else { "" }
            )>{title}</div>
            <div class="text-xs muted mt-1">{description}</div>
        </button>
    }
}

#[component]
fn ScopeButton(
    label: &'static str,
    active: impl Fn() -> bool + Send + Sync + 'static,
    on_click: impl Fn(leptos::ev::MouseEvent) + Send + Sync + 'static,
) -> impl IntoView {
    let active = StoredValue::new(active);
    let on_click = StoredValue::new(on_click);
    view! {
        <button
            type="button"
            class=move || format!(
                "pill {}",
                if active.with_value(|a| a()) { "pill-ok" } else { "" }
            )
            on:click=move |ev| on_click.with_value(|f| f(ev))
        >
            {label}
        </button>
    }
}

fn event_target_checked(ev: &leptos::ev::Event) -> bool {
    use leptos::web_sys;
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.checked())
        .unwrap_or(false)
}
