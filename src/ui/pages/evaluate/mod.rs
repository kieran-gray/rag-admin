use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};
use uuid::Uuid;

mod launchers;
mod redirect_by_id;
mod state;
mod steps;

pub use redirect_by_id::EvaluateByIdRedirect;
use state::EvaluateSelection;
use steps::{DatasetStep, ResultsStep, RunStep};

use crate::server_functions::configuration::{
    get_chunking_configurations, get_index_profiles, get_retrieval_profiles, get_sweep_templates,
};
use crate::server_functions::source_document::get_document_detail_by_source_ref;
use crate::shared::contracts::{
    aggregate_type, ChunkingConfigurationDto, IndexProfileDto, RetrievalProfileDto,
    SourceDocumentDetailDto, SourceDocumentDto, SweepTemplateDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};
use crate::ui::pages::shared::document_type_label;

#[component]
pub fn EvaluatePage() -> impl IntoView {
    let params = use_params_map();
    let source_ref =
        Memo::new(move |_| params.with(|p| p.get("source_ref").unwrap_or_default().to_string()));

    let query = use_query_map();
    let initial_step =
        Memo::new(move |_| query.with(|q| q.get("step").and_then(|t| step_from_query(&t))));
    let initial_dataset =
        Memo::new(move |_| query.with(|q| q.get("dataset").and_then(|v| Uuid::parse_str(&v).ok())));
    let initial_run =
        Memo::new(move |_| query.with(|q| q.get("run").and_then(|v| Uuid::parse_str(&v).ok())));

    let doc_invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let detail = Resource::new(
        move || (source_ref.get(), doc_invalidator.get()),
        move |(slug, _)| async move {
            if slug.is_empty() {
                return Err("missing source_ref".to_string());
            }
            get_document_detail_by_source_ref(slug)
                .await
                .map_err(|e| e.to_string())
        },
    );

    let index_profile_invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::VECTOR_INDEX_CATALOG,
            aggregate_type::SWEEP_TEMPLATE,
        ])
    });
    let index_profiles = Resource::new(
        move || index_profile_invalidator.get(),
        |_| async move { get_index_profiles().await.unwrap_or_default() },
    );
    let retrieval_profiles = Resource::new(
        move || index_profile_invalidator.get(),
        |_| async move { get_retrieval_profiles().await.unwrap_or_default() },
    );
    let chunking_configurations = Resource::new(
        move || index_profile_invalidator.get(),
        |_| async move { get_chunking_configurations().await.unwrap_or_default() },
    );
    let sweep_templates = Resource::new(
        move || index_profile_invalidator.get(),
        |_| async move { get_sweep_templates().await.unwrap_or_default() },
    );

    view! {
        <div>
            <Transition fallback=|| view! { <p class="muted">"Loading document…"</p> }>
                {move || {
                    let index_profiles = index_profiles.get().unwrap_or_default();
                    let retrieval_profiles = retrieval_profiles.get().unwrap_or_default();
                    let chunking_configurations = chunking_configurations.get().unwrap_or_default();
                    let sweep_templates = sweep_templates.get().unwrap_or_default();
                    detail.get().map(|res| match res {
                        Err(e) => view! {
                            <Surface>
                                <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                            </Surface>
                        }.into_any(),
                        Ok(None) => view! {
                            <UnregisteredDocument source_ref=source_ref.get() />
                        }.into_any(),
                        Ok(Some(existing)) => view! {
                            <EvaluateWorkspace
                                detail=existing
                                index_profiles=index_profiles
                                retrieval_profiles=retrieval_profiles
                                chunking_configurations=chunking_configurations
                                sweep_templates=sweep_templates
                                source_ref=source_ref.get()
                                initial_step=initial_step.get()
                                initial_dataset=initial_dataset.get()
                                initial_run=initial_run.get()
                            />
                        }.into_any(),
                    })
                }}
            </Transition>
        </div>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkflowStep {
    Dataset,
    Run,
    Results,
}

impl WorkflowStep {
    fn ordinal(self) -> u8 {
        match self {
            Self::Dataset => 1,
            Self::Run => 2,
            Self::Results => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Dataset => "Dataset",
            Self::Run => "Run",
            Self::Results => "Results",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Dataset => "Pick or generate questions",
            Self::Run => "Launch or pick a run",
            Self::Results => "Inspect scores",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepState {
    Current,
    Done,
    Available,
    Locked,
}

#[component]
fn EvaluateWorkspace(
    detail: SourceDocumentDetailDto,
    index_profiles: Vec<IndexProfileDto>,
    retrieval_profiles: Vec<RetrievalProfileDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    sweep_templates: Vec<SweepTemplateDto>,
    source_ref: String,
    initial_step: Option<WorkflowStep>,
    initial_dataset: Option<Uuid>,
    initial_run: Option<Uuid>,
) -> impl IntoView {
    let document_id = detail.document.document_id;
    let (header_eyebrow, header_title, header_subtitle) = derive_header(&detail.document);

    let selection = EvaluateSelection::new(
        &index_profiles,
        &retrieval_profiles,
        initial_dataset,
        initial_run,
    );

    let initial_step_value = initial_step.unwrap_or_else(|| {
        if initial_run.is_some() {
            WorkflowStep::Results
        } else if initial_dataset.is_some() {
            WorkflowStep::Run
        } else {
            WorkflowStep::Dataset
        }
    });
    let (active_step, set_active_step) = signal(initial_step_value);

    let index_profiles_stored = StoredValue::new(index_profiles);
    let chunking_stored = StoredValue::new(chunking_configurations);
    let sweep_templates_stored = StoredValue::new(sweep_templates);
    let source_ref_stored = StoredValue::new(source_ref);

    let step_state = move |step: WorkflowStep| -> StepState {
        let active = active_step.get();
        let has_dataset = selection.dataset_id.get().is_some();
        let has_run = selection.run_id.get().is_some();

        if step == active {
            return StepState::Current;
        }
        let locked = match step {
            WorkflowStep::Dataset => false,
            WorkflowStep::Run => !has_dataset,
            WorkflowStep::Results => !has_run,
        };
        if locked {
            return StepState::Locked;
        }
        let done = match step {
            WorkflowStep::Dataset => has_dataset,
            WorkflowStep::Run => has_run,
            WorkflowStep::Results => false,
        };
        if done {
            StepState::Done
        } else {
            StepState::Available
        }
    };

    let on_advance_to_run = Callback::new(move |_| set_active_step.set(WorkflowStep::Run));
    let on_advance_to_results = Callback::new(move |_| set_active_step.set(WorkflowStep::Results));
    let on_back_to_dataset = Callback::new(move |_| set_active_step.set(WorkflowStep::Dataset));
    let on_back_to_run = Callback::new(move |_| set_active_step.set(WorkflowStep::Run));

    view! {
        <div>
            <PageHeader
                title=header_title
                eyebrow=header_eyebrow
                subtitle=header_subtitle.unwrap_or_default()
            />

            <div class="space-y-6">
                <Stepper
                    set_active=set_active_step
                    step_state=step_state
                />

                {move || {
                    let step = active_step.get();
                    let source_ref = source_ref_stored.get_value();
                    let index_profiles = index_profiles_stored.get_value();
                    let chunking = chunking_stored.get_value();
                    let sweep = sweep_templates_stored.get_value();
                    match step {
                        WorkflowStep::Dataset => view! {
                            <DatasetStep
                                document_id=document_id
                                selection=selection
                                index_profiles=index_profiles_stored
                                on_advance=on_advance_to_run
                            />
                        }.into_any(),
                        WorkflowStep::Run => view! {
                            <RunStep
                                document_id=document_id
                                source_ref=&source_ref
                                selection=selection
                                index_profiles=index_profiles
                                chunking_configurations=chunking
                                sweep_templates=sweep
                                on_back=on_back_to_dataset
                                on_advance=on_advance_to_results
                            />
                        }.into_any(),
                        WorkflowStep::Results => view! {
                            <ResultsStep
                                selection=selection
                                on_back=on_back_to_run
                            />
                        }.into_any(),
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn Stepper(
    set_active: WriteSignal<WorkflowStep>,
    step_state: impl Fn(WorkflowStep) -> StepState + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let steps = [
        WorkflowStep::Dataset,
        WorkflowStep::Run,
        WorkflowStep::Results,
    ];

    view! {
        <div class="surface flex p-0 overflow-hidden">
            {steps.iter().enumerate().map(|(idx, step)| {
                let step = *step;
                let is_last = idx == steps.len() - 1;
                let state = move || step_state(step);
                let tooltip = move || match step {
                    WorkflowStep::Run => "Select a dataset first",
                    WorkflowStep::Results => "Launch or select a run first",
                    WorkflowStep::Dataset => "",
                };
                view! {
                    <button
                        type="button"
                        title=tooltip
                        class=move || {
                            let base = "flex-1 flex items-center gap-3 px-4 py-3 text-left transition-colors";
                            match state() {
                                StepState::Current => format!("{base} bg-[var(--color-surface-2)]"),
                                StepState::Locked => format!("{base} opacity-50 cursor-not-allowed"),
                                _ => format!("{base} hover:bg-[var(--color-surface-2)]"),
                            }
                        }
                        disabled=move || matches!(state(), StepState::Locked)
                        on:click=move |_| {
                            if !matches!(state(), StepState::Locked) {
                                set_active.set(step);
                            }
                        }
                    >
                        <span class=move || format!(
                            "inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-mono shrink-0 {}",
                            match state() {
                                StepState::Current => "bg-[var(--color-accent)] text-[var(--color-page-bg)]",
                                StepState::Done => "bg-[var(--color-accent-soft)] text-[var(--color-accent)] border border-[var(--color-accent)]",
                                StepState::Locked => "bg-transparent text-[var(--color-text-faint)] border border-[var(--color-border)]",
                                StepState::Available => "bg-[var(--color-surface-3)] text-[var(--color-text-muted)] border border-[var(--color-border)]",
                            }
                        )>
                            {move || if matches!(state(), StepState::Done) {
                                "✓".to_string()
                            } else {
                                step.ordinal().to_string()
                            }}
                        </span>
                        <div class="flex flex-col min-w-0">
                            <span class=move || format!(
                                "text-sm font-semibold {}",
                                match state() {
                                    StepState::Current => "text-[var(--color-text)]",
                                    StepState::Locked => "text-[var(--color-text-faint)]",
                                    _ => "text-[var(--color-text-muted)]",
                                }
                            )>
                                {step.label()}
                            </span>
                            <span class="text-[11px] muted">{step.hint()}</span>
                        </div>
                        {(!is_last).then(|| view! {
                            <span class="faint ml-auto pl-2">"›"</span>
                        })}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

fn step_from_query(s: &str) -> Option<WorkflowStep> {
    match s {
        "dataset" | "datasets" => Some(WorkflowStep::Dataset),
        "run" | "runs" => Some(WorkflowStep::Run),
        "results" | "result" => Some(WorkflowStep::Results),
        _ => None,
    }
}

fn derive_header(doc: &SourceDocumentDto) -> (String, String, Option<String>) {
    let type_label = document_type_label(&doc.document_type);
    let eyebrow = format!("Evaluate / {} / {}", type_label, doc.source_ref_key);
    let title = doc.title.clone();
    let subtitle = Some(format!(
        "{type_label} · v{} · evaluate this document",
        doc.latest_version,
    ));
    (eyebrow, title, subtitle)
}

#[component]
fn UnregisteredDocument(source_ref: String) -> impl IntoView {
    view! {
        <div>
            <PageHeader
                title=source_ref.clone()
                eyebrow=format!("Evaluate / {source_ref}")
                subtitle="No document is registered at this source ref.".to_string()
            />
            <Surface>
                <EmptyState
                    title="Not imported"
                    body="Import this document from the Documents page first; evaluations need a registered source.".to_string()
                />
            </Surface>
        </div>
    }
}
