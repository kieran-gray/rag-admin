use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use uuid::Uuid;

use crate::server_functions::evaluation::{
    get_runs_for_document, start_run_evaluation, start_run_optimization,
};
use crate::shared::contracts::{
    aggregate_type, ChunkingConfigurationDto, EvaluationRunSummaryDto, IndexProfileDto,
    RunEvaluationRequestDto, RunKindDto, RunOptimizationRequestDto, SweepTemplateDto,
};
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::evaluate::launchers::{
    EvaluationLauncher, IndexProfileSelect, LauncherCallbacks, OptimizeLauncher,
};
use crate::ui::pages::evaluate::state::EvaluateSelection;
use crate::ui::pages::shared::format_when;
use crate::ui::state::event_bus::use_invalidator;

#[component]
pub fn RunStep<'a>(
    document_id: Uuid,
    source_ref: &'a str,
    selection: EvaluateSelection,
    index_profiles: Vec<IndexProfileDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    sweep_templates: Vec<SweepTemplateDto>,
    on_back: Callback<()>,
) -> impl IntoView {
    let _ = source_ref;
    let index_profiles_stored = StoredValue::new(index_profiles);
    let chunking_stored = StoredValue::new(chunking_configurations);
    let sweep_stored = StoredValue::new(sweep_templates);

    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let runs = Resource::new(
        move || (document_id, invalidator.get()),
        move |(id, _)| async move { get_runs_for_document(id).await.unwrap_or_default() },
    );

    let dataset_runs: Signal<Vec<EvaluationRunSummaryDto>> = Signal::derive(move || {
        let Some(ds) = selection.dataset_id.get() else {
            return Vec::new();
        };
        runs.get()
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.dataset_id == ds)
            .collect()
    });

    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let go_to_run = move |run_id: Uuid, _kind: RunKindDto| {
        selection.run_id.set(Some(run_id));
        use_navigate()(
            &format!("/workflows/runs/{run_id}"),
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    };

    let launch_optimization = Callback::new(move |req: RunOptimizationRequestDto| {
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match start_run_optimization(req).await {
                Ok(info) => {
                    set_busy.set(false);
                    match info.job_id.parse::<Uuid>() {
                        Ok(run_id) => go_to_run(run_id, RunKindDto::Optimization),
                        Err(_) => set_error.set(Some(format!(
                            "Server returned unexpected run id: {}",
                            info.job_id
                        ))),
                    }
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    });

    let launch_manual = Callback::new(move |req: RunEvaluationRequestDto| {
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match start_run_evaluation(req).await {
                Ok(info) => {
                    set_busy.set(false);
                    match info.job_id.parse::<Uuid>() {
                        Ok(run_id) => go_to_run(run_id, RunKindDto::Manual),
                        Err(_) => set_error.set(Some(format!(
                            "Server returned unexpected run id: {}",
                            info.job_id
                        ))),
                    }
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    });

    let active_dataset = selection.dataset_id.read_only();
    let active_index_profile = selection.index_profile_id.read_only();
    let set_active_index_profile = selection.index_profile_id.write_only();

    view! {
        <div class="space-y-6">
            <Surface>
                <IndexProfileSelect
                    index_profiles=index_profiles_stored
                    value=active_index_profile
                    set_value=set_active_index_profile
                />
            </Surface>

            <Surface
                title="Runs on this dataset".to_string()
                actions=Box::new(move || view! {
                    <Help title="Picking versus launching".to_string()>
                        <p>
                            "Each row below is a past run against the selected dataset. Open one to jump to its results, or launch a new run below to compare more configurations."
                        </p>
                    </Help>
                }.into_any())
            >
                <Transition fallback=|| view! { <p class="muted text-sm">"Loading runs…"</p> }>
                    {move || {
                        let list = dataset_runs.get();
                        if list.is_empty() {
                            return view! {
                                <EmptyState
                                    title="No runs on this dataset yet"
                                    body="Use the launcher below to find the best configuration or score a chosen variant.".to_string()
                                />
                            }.into_any();
                        }
                        view! {
                            <div class="space-y-2">
                                {list.into_iter().map(|r| view! { <RunRow run=r /> }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </Transition>
            </Surface>

            {move || error.get().map(|e| view! {
                <Surface>
                    <div class="log-line-error text-sm">{e}</div>
                </Surface>
            })}

            {move || view! {
                <OptimizeLauncher
                    dataset_id=selection.dataset_id.get()
                    index_profile_id=selection.index_profile_id.get()
                    chunking_configurations=chunking_stored
                    on_launch=launch_optimization
                    busy=busy
                    error=error
                />
            }}

            <details class="surface-raised rounded p-4">
                <summary class="cursor-pointer text-sm muted hover:text-text">
                    "Score specific variants instead"
                </summary>
                <div class="pt-4">
                    <EvaluationLauncher
                        chunking_configurations=chunking_stored
                        sweep_templates=sweep_stored
                        active_dataset=active_dataset
                        active_index_profile=active_index_profile
                        running=busy
                        callbacks=LauncherCallbacks { on_start: launch_manual }
                    />
                </div>
            </details>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Back"</span>
                    <span class="step-advance-eyebrow-label">"Change dataset"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-ghost"
                    on:click=move |_| on_back.run(())
                >
                    "← Back"
                </button>
            </div>
        </div>
    }
}

#[component]
fn RunRow(run: EvaluationRunSummaryDto) -> impl IntoView {
    let (kind, label) = run_status(&run.status);
    let when = format_when(&run.created_at);
    let when_full = run.created_at.clone();
    let run_short = run.run_id.to_string().chars().take(8).collect::<String>();
    let variant_count = run.variant_count;
    let run_id = run.run_id;

    view! {
        <A
            href=format!("/workflows/runs/{run_id}")
            attr:class="w-full flex items-center justify-between gap-3 px-3 py-2 rounded border border-[var(--color-border)] hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-2)] transition-colors no-underline"
        >
            <span class="flex-1 flex items-center gap-3">
                <span class="font-mono text-sm">{format!("run-{run_short}")}</span>
                <span class="text-xs muted">{format!("{variant_count} variants")}</span>
            </span>
            <span class="flex items-center gap-3">
                <StatusPill label=label.to_string() kind=kind />
                <span class="text-xs faint whitespace-nowrap" title=when_full>{when}</span>
                <span class="faint" aria-hidden="true">"›"</span>
            </span>
        </A>
    }
}

fn run_status(status: &str) -> (Status, &'static str) {
    match status {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        "pending" => (Status::Pending, "Pending"),
        "cancelled" => (Status::Cancel, "Cancelled"),
        _ => (Status::Neutral, "Unknown"),
    }
}
