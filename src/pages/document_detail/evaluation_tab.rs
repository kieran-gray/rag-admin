use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use uuid::Uuid;

use crate::components::activity::toggle_drawer;
use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, Status, StatusPill, Surface};
use crate::server_functions::evaluation::{
    get_datasets_for_document, get_runs_for_document, start_generate_synthetic_dataset,
    start_run_evaluation,
};
use crate::shared::{
    aggregate_type, ChunkingConfigurationDto, EvaluationDatasetSummaryDto, EvaluationRunSummaryDto,
    PipelineConfigurationDto, RunEvaluationRequestDto, SourceDocumentDetailDto, SweepTemplateDto,
};

use super::eval_launcher::{EvaluationLauncher, LauncherCallbacks};

#[component]
pub fn EvaluationTab(
    detail: Option<SourceDocumentDetailDto>,
    pipelines: Vec<PipelineConfigurationDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    sweep_templates: Vec<SweepTemplateDto>,
) -> impl IntoView {
    let Some(detail) = detail else {
        return view! {
            <Surface>
                <EmptyState
                    title="Document not imported"
                    body="Import this document from its source first; evaluations run their own chunk+embed in memory and don't require an indexing.".to_string()
                />
            </Surface>
        }.into_any();
    };
    let document_id = detail.document.document_id;
    let pipelines_stored = StoredValue::new(pipelines);
    let chunking_stored = StoredValue::new(chunking_configurations);
    let sweep_templates_stored = StoredValue::new(sweep_templates);

    view! {
        <EvaluationWorkspace
            document_id=document_id
            pipelines=pipelines_stored
            chunking_configurations=chunking_stored
            sweep_templates=sweep_templates_stored
        />
    }
    .into_any()
}

#[component]
fn EvaluationWorkspace(
    document_id: Uuid,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
) -> impl IntoView {
    let dataset_invalidator =
        use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_DATASET]));
    let datasets = Resource::new(
        move || dataset_invalidator.get(),
        move |_| async move {
            get_datasets_for_document(document_id)
                .await
                .unwrap_or_default()
        },
    );

    let run_invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let runs = Resource::new(
        move || run_invalidator.get(),
        move |_| async move { get_runs_for_document(document_id).await.unwrap_or_default() },
    );

    let (active_dataset, set_active_dataset) = signal::<Option<Uuid>>(None);
    let (active_pipeline, set_active_pipeline) = signal::<Option<Uuid>>(None);

    Effect::new(move |_| {
        if active_dataset.get_untracked().is_none() {
            if let Some(list) = datasets.get() {
                if let Some(first) = list.first() {
                    set_active_dataset.set(Some(first.dataset_id));
                }
            }
        }
    });
    Effect::new(move |_| {
        if active_pipeline.get_untracked().is_none() {
            let list = pipelines.get_value();
            if let Some(first) = list.first() {
                set_active_pipeline.set(Some(first.pipeline_configuration_id));
            }
        }
    });

    let (job_running, set_job_running) = signal(false);
    let (launch_error, set_launch_error) = signal::<Option<String>>(None);
    let (dataset_label, set_dataset_label) = signal("synthetic-default".to_string());

    let on_generate = move |_| {
        let label = dataset_label.get();
        let Some(pipeline_configuration_id) = active_pipeline.get() else {
            set_launch_error.set(Some(
                "Select a pipeline configuration before generating a synthetic dataset."
                    .to_string(),
            ));
            return;
        };
        set_launch_error.set(None);
        set_job_running.set(true);
        spawn_local(async move {
            match start_generate_synthetic_dataset(document_id, pipeline_configuration_id, label)
                .await
            {
                Ok(_job) => {
                    toggle_drawer(true);
                    set_job_running.set(false);
                }
                Err(e) => {
                    set_job_running.set(false);
                    set_launch_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let on_start_run = Callback::new(move |request: RunEvaluationRequestDto| {
        set_launch_error.set(None);
        set_job_running.set(true);
        spawn_local(async move {
            match start_run_evaluation(request).await {
                Ok(_job) => {
                    toggle_drawer(true);
                    set_job_running.set(false);
                }
                Err(e) => {
                    set_job_running.set(false);
                    set_launch_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    view! {
        <div class="space-y-6">
            <Surface title="Dataset".to_string()>
                <Transition fallback=|| view! { <p class="muted">"Loading datasets…"</p> }>
                    {move || {
                        let list = datasets.get().unwrap_or_default();
                        if list.is_empty() {
                            return view! {
                                <DatasetGenerateForm
                                    dataset_label=dataset_label
                                    set_dataset_label=set_dataset_label
                                    on_generate=Box::new(on_generate)
                                    running=job_running
                                />
                            }.into_any();
                        }
                        view! {
                            <div class="space-y-3">
                                {list.into_iter().map(|d| {
                                    let did = d.dataset_id;
                                    let active = move || active_dataset.get() == Some(did);
                                    view! {
                                        <DatasetRow
                                            dataset=d
                                            is_active=active
                                            on_select=move || set_active_dataset.set(Some(did))
                                        />
                                    }
                                }).collect_view()}
                                <div class="pt-2 border-t border-[var(--color-border)]">
                                    <DatasetGenerateForm
                                        dataset_label=dataset_label
                                        set_dataset_label=set_dataset_label
                                        on_generate=Box::new(on_generate)
                                        running=job_running
                                    />
                                </div>
                            </div>
                        }.into_any()
                    }}
                </Transition>
            </Surface>

            <EvaluationLauncher
                pipelines=pipelines
                chunking_configurations=chunking_configurations
                sweep_templates=sweep_templates
                active_dataset=active_dataset
                active_pipeline=active_pipeline
                set_active_pipeline=set_active_pipeline
                running=job_running
                callbacks=LauncherCallbacks { on_start: on_start_run }
            />

            {move || launch_error.get().map(|err| view! {
                <Surface>
                    <div class="log-line-error text-sm">{err}</div>
                </Surface>
            })}

            <Surface title="Runs".to_string()>
                <Transition fallback=|| view! { <p class="muted">"Loading runs…"</p> }>
                    {move || {
                        let list = runs.get().unwrap_or_default();
                        if list.is_empty() {
                            return view! {
                                <EmptyState
                                    title="No runs yet"
                                    body="Launch a tune-for-best run above; results land here as they complete.".to_string()
                                />
                            }.into_any();
                        }
                        view! {
                            <div class="space-y-3">
                                {list.into_iter().map(|r| view! { <RunCard run=r /> }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </Transition>
            </Surface>
        </div>
    }
}

#[component]
fn DatasetGenerateForm(
    dataset_label: ReadSignal<String>,
    set_dataset_label: WriteSignal<String>,
    on_generate: Box<dyn Fn(leptos::ev::MouseEvent) + Send + Sync>,
    running: ReadSignal<bool>,
) -> impl IntoView {
    let on_generate = StoredValue::new(on_generate);
    view! {
        <div class="flex items-center gap-2">
            <span class="eyebrow shrink-0">"Generate"</span>
            <input
                class="input"
                placeholder="dataset label"
                prop:value=move || dataset_label.get()
                on:input=move |ev| set_dataset_label.set(event_target_value(&ev))
            />
            <button
                type="button"
                class="btn"
                disabled=move || running.get()
                on:click=move |ev| on_generate.with_value(|f| f(ev))
            >
                {move || if running.get() { "Generating…" } else { "Generate" }}
            </button>
        </div>
    }
}

#[component]
fn DatasetRow<F>(
    dataset: EvaluationDatasetSummaryDto,
    is_active: F,
    on_select: impl Fn() + Send + Sync + 'static,
) -> impl IntoView
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    let on_select_stored = StoredValue::new(on_select);
    let (kind, label) = eval_status(&dataset.status);
    let q_count = dataset.question_count;
    let dataset_label = dataset.label;
    let dataset_id = dataset.dataset_id;
    view! {
        <div
            class=move || format!(
                "w-full flex items-center justify-between gap-3 px-3 py-2 rounded border transition-colors {}",
                if is_active() {
                    "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                } else {
                    "border-[var(--color-border)] hover:border-[var(--color-border-strong)]"
                }
            )
        >
            <button
                type="button"
                class="flex-1 text-left text-text"
                on:click=move |_| on_select_stored.with_value(|f| f())
            >
                {dataset_label}
            </button>
            <div class="flex items-center gap-3">
                <span class="text-xs muted">{format!("{q_count} questions")}</span>
                <StatusPill label=label.to_string() kind=kind />
                <A
                    href=format!("/datasets/{dataset_id}")
                    attr:class="text-xs muted hover:text-text underline-offset-2 hover:underline"
                >
                    "View →"
                </A>
            </div>
        </div>
    }
}

#[component]
fn RunCard(run: EvaluationRunSummaryDto) -> impl IntoView {
    let (kind, label) = eval_status(&run.status);
    let when = run
        .created_at
        .get(..16)
        .unwrap_or(&run.created_at)
        .to_string();
    let run_short = run.run_id.to_string()[..8].to_string();
    let variant_count = run.variant_count;
    let run_id = run.run_id;

    view! {
        <div class="surface-raised rounded p-4 space-y-2">
            <div class="flex items-center justify-between gap-3">
                <div class="flex items-center gap-3">
                    <A href=format!("/runs/{run_id}")>
                        <span class="font-mono text-sm">{format!("run-{run_short}")}</span>
                    </A>
                    <StatusPill label=label.to_string() kind=kind />
                    <span class="text-xs muted">{format!("{variant_count} variants")}</span>
                </div>
                <span class="text-xs faint font-mono">{when}</span>
            </div>
            <div class="text-xs muted">"Open the run to compare variants."</div>
        </div>
    }
}

fn eval_status(status: &str) -> (Status, &'static str) {
    match status {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        "generating" => (Status::Pending, "Generating"),
        "pending" => (Status::Pending, "Pending"),
        _ => (Status::Neutral, "Unknown"),
    }
}
