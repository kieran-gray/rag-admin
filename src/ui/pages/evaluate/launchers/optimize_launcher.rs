use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{ChunkingConfigurationDto, RunOptimizationRequestDto};
use crate::shared::{OptimizationBudget, OptimizationConfig, OptimizationScope};
use crate::ui::components::primitives::{Status, StatusPill, Surface};

fn describe_budget(b: OptimizationBudget) -> String {
    b.describe().to_string()
}

#[component]
pub fn OptimizeLauncher(
    dataset_id: Option<Uuid>,
    index_profile_id: Option<Uuid>,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    on_launch: Callback<RunOptimizationRequestDto>,
    busy: ReadSignal<bool>,
    #[prop(optional)] error: Option<ReadSignal<Option<String>>>,
) -> impl IntoView {
    let (budget, set_budget) = signal(OptimizationBudget::Thorough);
    let (scope, set_scope) = signal(OptimizationScope::Both);
    let (judges, set_judges) = signal(false);
    let (local_error, set_local_error) = signal::<Option<String>>(None);

    let default_pinned_id = chunking_configurations.with_value(|cs| {
        cs.iter()
            .find(|c| c.is_default)
            .map(|c| c.chunking_configuration_id)
            .or_else(|| cs.first().map(|c| c.chunking_configuration_id))
    });
    let (pinned_chunking_id, set_pinned_chunking_id) = signal::<Option<Uuid>>(default_pinned_id);

    let on_click = move |_| {
        let Some(ds) = dataset_id else {
            set_local_error.set(Some(
                "Pick a dataset before launching an optimization.".into(),
            ));
            return;
        };
        let Some(profile) = index_profile_id else {
            set_local_error.set(Some(
                "Pick an index profile before launching an optimization.".into(),
            ));
            return;
        };
        set_local_error.set(None);
        let scope_now = scope.get_untracked();
        let fixed_chunking_configuration_id = if scope_now == OptimizationScope::Retrieval {
            pinned_chunking_id.get_untracked()
        } else {
            None
        };
        if scope_now == OptimizationScope::Retrieval && fixed_chunking_configuration_id.is_none() {
            set_local_error.set(Some(
                "Pin a chunking configuration before launching a retrieval-scope optimization."
                    .into(),
            ));
            return;
        }
        on_launch.run(RunOptimizationRequestDto {
            dataset_id: ds,
            index_profile_id: profile,
            retrieval_profile_id: None,
            optimization: OptimizationConfig {
                budget: budget.get_untracked(),
                scope: scope_now,
                judges_enabled: judges.get_untracked(),
                seed: None,
                fixed_chunking_configuration_id,
            },
        });
    };

    let surfaced_error = move || error.and_then(|e| e.get()).or_else(|| local_error.get());

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

            {move || (scope.get() == OptimizationScope::Retrieval).then(|| view! {
                <div class="flex items-center gap-3 mb-3">
                    <div class="eyebrow shrink-0">"Pin chunking config"</div>
                    <select
                        class="input max-w-md"
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            if v.is_empty() {
                                set_pinned_chunking_id.set(None);
                            } else if let Ok(id) = v.parse::<Uuid>() {
                                set_pinned_chunking_id.set(Some(id));
                            }
                        }
                    >
                        {move || {
                            let configs = chunking_configurations.get_value();
                            let selected_id = pinned_chunking_id.get();
                            configs.into_iter().map(|c| {
                                let id = c.chunking_configuration_id;
                                let selected = Some(id) == selected_id;
                                let label = if c.is_default {
                                    format!("{} (default)", c.name)
                                } else {
                                    c.name
                                };
                                view! {
                                    <option value=id.to_string() selected=selected>
                                        {label}
                                    </option>
                                }
                            }).collect_view()
                        }}
                    </select>
                </div>
            })}

            <div class="flex items-center justify-between items-end gap-4 mt-4 mb-2 pt-4 border-t border-[var(--color-border)]">
                <label class="flex items-center gap-2 text-sm muted">
                    <input
                        type="checkbox"
                        prop:checked=move || judges.get()
                        on:change=move |ev| set_judges.set(event_target_checked(&ev))
                    />
                    "Enable LLM-judge pass on final rung (slow; extra cost)"
                </label>

                {move || surfaced_error().map(|e| view! {
                    <StatusPill label=format!("Error: {e}") kind=Status::Fail />
                })}

                <button
                    type="button"
                    class="btn btn-primary"
                    prop:disabled=move || busy.get()
                    on:click=on_click
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
