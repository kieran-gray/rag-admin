use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_params_map, use_query_map};

mod steps;

use steps::{ChunkStep, ConfigSelection, DocumentStep, EmbedStep, IndexStep};

use super::shared::short_hash;

use crate::server_functions::configuration::{
    get_chunking_configurations, get_pipeline_configurations,
};
use crate::server_functions::source_document::get_document_detail_by_source_ref;
use crate::shared::contracts::{
    aggregate_type, ChunkingConfigurationDto, IndexingDto, PipelineConfigurationDto,
    SourceDocumentDetailDto, SourceDocumentDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};

#[component]
pub fn DocumentDetailPage() -> impl IntoView {
    let params = use_params_map();
    let source_ref =
        Memo::new(move |_| params.with(|p| p.get("source_ref").unwrap_or_default().to_string()));

    let query = use_query_map();
    let initial_tab =
        Memo::new(move |_| query.with(|q| q.get("tab").and_then(|t| step_from_tab(&t))));

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

    let pipeline_invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::VECTOR_INDEX_CATALOG,
        ])
    });
    let pipelines = Resource::new(
        move || pipeline_invalidator.get(),
        |_| async move { get_pipeline_configurations().await.unwrap_or_default() },
    );
    let chunking_configurations = Resource::new(
        move || pipeline_invalidator.get(),
        |_| async move { get_chunking_configurations().await.unwrap_or_default() },
    );
    view! {
        <div>
            <Transition fallback=|| view! {
                <p class="muted">"Loading document…"</p>
            }>
                {move || {
                    let pipelines = pipelines.get().unwrap_or_default();
                    let chunking_configurations = chunking_configurations.get().unwrap_or_default();
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
                            <DocumentWorkspace
                                detail=existing
                                pipelines=pipelines
                                chunking_configurations=chunking_configurations
                                source_ref=source_ref.get()
                                initial_tab=initial_tab.get()
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
    Document,
    Chunk,
    Embed,
    Index,
}

impl WorkflowStep {
    fn ordinal(self) -> u8 {
        match self {
            Self::Document => 1,
            Self::Chunk => 2,
            Self::Embed => 3,
            Self::Index => 4,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Chunk => "Chunk",
            Self::Embed => "Embed",
            Self::Index => "Index",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Document => "Review markdown",
            Self::Chunk => "Split into chunks",
            Self::Embed => "Vectorize chunks",
            Self::Index => "Upsert to vector store",
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
fn DocumentWorkspace(
    detail: SourceDocumentDetailDto,
    pipelines: Vec<PipelineConfigurationDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    source_ref: String,
    initial_tab: Option<WorkflowStep>,
) -> impl IntoView {
    let (header_eyebrow, header_title, header_subtitle, header_status) =
        derive_header(&detail.document, &detail.indexings);
    let (status_kind, status_label) = header_status;

    let selection = ConfigSelection::new(&pipelines, &chunking_configurations, &detail.indexings);
    let initial_step_value = initial_tab.unwrap_or_else(|| initial_step(&detail.indexings));
    let (active_step, set_active_step) = signal(initial_step_value);

    let pipelines_stored = StoredValue::new(pipelines.clone());
    let chunking_stored = StoredValue::new(chunking_configurations.clone());
    let indexings_stored = StoredValue::new(detail.indexings.clone());
    let indexings_signal: Signal<Vec<IndexingDto>> =
        Signal::derive(move || indexings_stored.get_value());

    let active_indexing: Signal<Option<IndexingDto>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        indexings_signal
            .get()
            .into_iter()
            .find(|ix| ix.pipeline_configuration_id == pid && !ix.removed)
    });

    let step_state = move |step: WorkflowStep| -> StepState {
        let active = active_step.get();
        let ix = active_indexing.get();
        let has_chunk = ix.as_ref().is_some_and(|i| i.chunk_set_id.is_some());
        let has_embed = ix.as_ref().is_some_and(|i| i.embedding_set_id.is_some());
        let is_indexed = ix.as_ref().is_some_and(|i| i.status.contains("Indexed"));

        if step == active {
            return StepState::Current;
        }
        let locked = match step {
            WorkflowStep::Document | WorkflowStep::Chunk => false,
            WorkflowStep::Embed => !has_chunk,
            WorkflowStep::Index => !has_embed,
        };
        if locked {
            return StepState::Locked;
        }
        let done = match step {
            WorkflowStep::Document => true,
            WorkflowStep::Chunk => has_chunk,
            WorkflowStep::Embed => has_embed,
            WorkflowStep::Index => is_indexed,
        };
        if done {
            StepState::Done
        } else {
            StepState::Available
        }
    };

    let evaluate_href = format!(
        "/evaluate/{}/{}",
        detail.document.document_type.to_lowercase(),
        urlencoding::encode(&source_ref),
    );
    let source_ref_for_step = StoredValue::new(source_ref);

    let on_advance_to_chunk = Callback::new(move |_| set_active_step.set(WorkflowStep::Chunk));
    let on_advance_to_embed = Callback::new(move |_| set_active_step.set(WorkflowStep::Embed));
    let on_advance_to_index = Callback::new(move |_| set_active_step.set(WorkflowStep::Index));
    let on_back_to_chunk = Callback::new(move |_| set_active_step.set(WorkflowStep::Chunk));
    let on_back_to_embed = Callback::new(move |_| set_active_step.set(WorkflowStep::Embed));

    let header_subtitle = header_subtitle.unwrap_or_default();

    view! {
        <div>
            <PageHeader
                title=header_title
                eyebrow=header_eyebrow
                subtitle=header_subtitle
                actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <StatusPill label=status_label.clone() kind=status_kind />
                        <A
                            href=evaluate_href.clone()
                            attr:class="btn btn-ghost"
                        >
                            "Evaluate →"
                        </A>
                    </div>
                }.into_any())
            />

            <div class="space-y-6">
                <Stepper
                    set_active=set_active_step
                    step_state=step_state
                />

                {move || {
                    let step = active_step.get();
                    let source_ref = source_ref_for_step.get_value();
                    match step {
                        WorkflowStep::Document => view! {
                            <DocumentStep
                                selection=selection
                                pipelines=pipelines_stored
                                chunking_configurations=chunking_stored
                                indexings=indexings_signal
                                source_ref=source_ref
                                on_advance=on_advance_to_chunk
                            />
                        }.into_any(),
                        WorkflowStep::Chunk => view! {
                            <ChunkStep
                                selection=selection
                                chunking_configurations=chunking_stored
                                indexings=indexings_signal
                                source_ref=source_ref
                                on_advance=on_advance_to_embed
                            />
                        }.into_any(),
                        WorkflowStep::Embed => view! {
                            <EmbedStep
                                selection=selection
                                pipelines=pipelines_stored
                                indexings=indexings_signal
                                on_back=on_back_to_chunk
                                on_advance=on_advance_to_index
                            />
                        }.into_any(),
                        WorkflowStep::Index => view! {
                            <IndexStep
                                selection=selection
                                pipelines=pipelines_stored
                                indexings=indexings_signal
                                on_back=on_back_to_embed
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
        WorkflowStep::Document,
        WorkflowStep::Chunk,
        WorkflowStep::Embed,
        WorkflowStep::Index,
    ];

    view! {
        <div class="surface flex p-0 overflow-hidden">
            {steps.iter().enumerate().map(|(idx, step)| {
                let step = *step;
                let is_last = idx == steps.len() - 1;
                let state = move || step_state(step);
                let tooltip = move || match step {
                    WorkflowStep::Embed => "Chunk the document first",
                    WorkflowStep::Index => "Embed the chunks first",
                    _ => "",
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

fn step_from_tab(tab: &str) -> Option<WorkflowStep> {
    match tab {
        "source" | "document" => Some(WorkflowStep::Document),
        "chunk" | "chunks" | "chunking" => Some(WorkflowStep::Chunk),
        "embed" | "embedding" => Some(WorkflowStep::Embed),
        "index" | "indexing" => Some(WorkflowStep::Index),
        _ => None,
    }
}

fn initial_step(indexings: &[IndexingDto]) -> WorkflowStep {
    let live: Vec<&IndexingDto> = indexings.iter().filter(|ix| !ix.removed).collect();
    if live.is_empty() {
        return WorkflowStep::Document;
    }
    let most_advanced = live.iter().copied().max_by_key(|ix| milestone_rank(ix));
    match most_advanced {
        None => WorkflowStep::Document,
        Some(ix) if ix.status.contains("Indexed") => WorkflowStep::Index,
        Some(ix) if ix.embedding_set_id.is_some() => WorkflowStep::Index,
        Some(ix) if ix.chunk_set_id.is_some() => WorkflowStep::Embed,
        _ => WorkflowStep::Chunk,
    }
}

fn derive_header(
    doc: &SourceDocumentDto,
    indexings: &[IndexingDto],
) -> (String, String, Option<String>, (Status, String)) {
    let type_label = document_type_label(&doc.document_type);
    let eyebrow = format!("Documents / {} / {}", type_label, doc.source_ref_key);
    let title = doc.title.clone();
    let subtitle = Some(format!(
        "{type_label} · v{} · {}",
        doc.latest_version,
        short_hash(&doc.latest_content_hash),
    ));
    let status = derive_status(indexings, doc.latest_version);
    (eyebrow, title, subtitle, status)
}

fn derive_status(indexings: &[IndexingDto], latest_version: u32) -> (Status, String) {
    let live: Vec<&IndexingDto> = indexings.iter().filter(|ix| !ix.removed).collect();
    if live.is_empty() {
        return (Status::Stale, "Not indexed".to_string());
    }
    if live.iter().any(|i| i.status.contains("Failed")) {
        return (Status::Fail, "Has failures".to_string());
    }

    let indexed: Vec<&IndexingDto> = live
        .iter()
        .copied()
        .filter(|i| i.status.contains("Indexed"))
        .collect();

    if !indexed.is_empty() {
        let any_stale = indexed.iter().any(|i| i.document_version < latest_version);
        return if any_stale {
            (
                Status::Pending,
                format!("Indexed · stale (v{latest_version})"),
            )
        } else if indexed.len() == 1 {
            (Status::Ok, "Indexed".to_string())
        } else {
            (Status::Ok, format!("Indexed × {}", indexed.len()))
        };
    }

    let Some(most_advanced) = live.iter().copied().max_by_key(|ix| milestone_rank(ix)) else {
        return (Status::Stale, "Not indexed".to_string());
    };

    match (most_advanced.chunk_set_id, most_advanced.embedding_set_id) {
        (None, _) => (Status::Pending, "Chunking…".to_string()),
        (Some(_), None) => (Status::Info, "Chunked".to_string()),
        (Some(_), Some(_)) => (Status::Info, "Embedded".to_string()),
    }
}

fn milestone_rank(ix: &IndexingDto) -> u8 {
    if ix.status.contains("Indexed") {
        4
    } else if ix.embedding_set_id.is_some() {
        3
    } else if ix.chunk_set_id.is_some() {
        2
    } else {
        1
    }
}

fn document_type_label(doc_type: &str) -> &'static str {
    match doc_type {
        "Markdown" => "Markdown",
        "PlainText" => "Plain text",
        "WebPage" => "Web page",
        _ => "Document",
    }
}

#[component]
fn UnregisteredDocument(source_ref: String) -> impl IntoView {
    view! {
        <div>
            <PageHeader
                title=source_ref.clone()
                eyebrow=format!("Documents / {source_ref}")
                subtitle="No document is registered at this source ref.".to_string()
            />
            <Surface>
                <EmptyState
                    title="Not imported"
                    body="Open the Documents page and import this from a connector, URL, or upload.".to_string()
                />
            </Surface>
        </div>
    }
}
