use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

mod redirect_by_id;
mod search_section;
mod steps;
mod tabs;

pub use redirect_by_id::DocumentByIdRedirect;

use search_section::SearchSection;
use steps::{ChunkStep, ConfigSelection, DocumentStep, EmbedIndexStep};

use super::shared::short_hash;

use crate::contracts::{
    aggregate_type, ChunkingConfigurationDto, IndexingDto, PipelineConfigurationDto,
    SourceDocumentDetailDto, SourceDocumentDto, SweepTemplateDto,
};
use crate::server_functions::configuration::{
    get_chunking_configurations, get_pipeline_configurations, get_sweep_templates,
};
use crate::server_functions::source_document::{
    get_document_detail_by_source_ref, import_source_document, start_indexing_with_defaults,
};
use crate::ui::components::activity::toggle_drawer;
use crate::ui::components::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};

#[component]
pub fn DocumentDetailPage() -> impl IntoView {
    let params = use_params_map();
    let source_ref =
        Memo::new(move |_| params.with(|p| p.get("source_ref").unwrap_or_default().to_string()));

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
            aggregate_type::SWEEP_TEMPLATE,
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
    let sweep_templates = Resource::new(
        move || pipeline_invalidator.get(),
        |_| async move { get_sweep_templates().await.unwrap_or_default() },
    );

    view! {
        <div>
            <Transition fallback=|| view! {
                <p class="muted">"Loading document…"</p>
            }>
                {move || {
                    let pipelines = pipelines.get().unwrap_or_default();
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
                            <DocumentWorkspace
                                detail=existing
                                pipelines=pipelines
                                chunking_configurations=chunking_configurations
                                sweep_templates=sweep_templates
                                source_ref=source_ref.get()
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
    EmbedIndex,
}

impl WorkflowStep {
    fn ordinal(self) -> u8 {
        match self {
            Self::Document => 1,
            Self::Chunk => 2,
            Self::EmbedIndex => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Document => "Document",
            Self::Chunk => "Chunk",
            Self::EmbedIndex => "Embed & Index",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Document => "Review markdown",
            Self::Chunk => "Split into chunks",
            Self::EmbedIndex => "Vectorize & store",
        }
    }
}

#[component]
fn DocumentWorkspace(
    detail: SourceDocumentDetailDto,
    pipelines: Vec<PipelineConfigurationDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    sweep_templates: Vec<SweepTemplateDto>,
    source_ref: String,
) -> impl IntoView {
    let document_id = detail.document.document_id;

    let (header_eyebrow, header_title, header_subtitle, header_status) =
        derive_header(&detail.document, &detail.indexings);
    let (status_kind, status_label) = header_status;

    let selection = ConfigSelection::new(&pipelines, &chunking_configurations, &detail.indexings);
    let (active_step, set_active_step) = signal(initial_step(&detail.indexings));

    let pipelines_stored = StoredValue::new(pipelines.clone());
    let chunking_stored = StoredValue::new(chunking_configurations.clone());
    let indexings_stored = StoredValue::new(detail.indexings.clone());
    let indexings_signal: Signal<Vec<IndexingDto>> =
        Signal::derive(move || indexings_stored.get_value());

    let detail_for_step = StoredValue::new(detail);
    let source_ref_for_step = StoredValue::new(source_ref);
    let sweep_for_step = StoredValue::new(sweep_templates);

    let on_advance_to_chunk = Callback::new(move |_| set_active_step.set(WorkflowStep::Chunk));
    let on_advance_to_embed = Callback::new(move |_| set_active_step.set(WorkflowStep::EmbedIndex));
    let on_back_to_chunk = Callback::new(move |_| set_active_step.set(WorkflowStep::Chunk));

    let header_subtitle = header_subtitle.unwrap_or_default();
    let source_ref_for_quick = source_ref_for_step;

    view! {
        <div>
            <PageHeader
                title=header_title
                eyebrow=header_eyebrow
                subtitle=header_subtitle
                actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <StatusPill label=status_label.clone() kind=status_kind />
                        <QuickIndexButton source_ref=source_ref_for_quick />
                    </div>
                }.into_any())
            />

            <div class="space-y-6">
                <Stepper active=active_step set_active=set_active_step />

                {move || {
                    let step = active_step.get();
                    let detail = detail_for_step.get_value();
                    let source_ref = source_ref_for_step.get_value();
                    let pipelines = pipelines_stored.get_value();
                    let chunking = chunking_stored.get_value();
                    let sweep = sweep_for_step.get_value();
                    match step {
                        WorkflowStep::Document => view! {
                            <DocumentStep
                                detail=detail
                                pipelines=pipelines
                                chunking_configurations=chunking
                                sweep_templates=sweep
                                source_ref=source_ref
                                on_advance=on_advance_to_chunk
                            />
                        }.into_any(),
                        WorkflowStep::Chunk => view! {
                            <ChunkStep
                                selection=selection
                                pipelines=pipelines_stored
                                chunking_configurations=chunking_stored
                                indexings=indexings_signal
                                source_ref=source_ref
                                on_advance=on_advance_to_embed
                            />
                        }.into_any(),
                        WorkflowStep::EmbedIndex => view! {
                            <EmbedIndexStep
                                selection=selection
                                pipelines=pipelines_stored
                                indexings=indexings_signal
                                on_back=on_back_to_chunk
                            />
                        }.into_any(),
                    }
                }}

                {move || {
                    let on_embed_step = matches!(active_step.get(), WorkflowStep::EmbedIndex);
                    let any_indexed = indexings_signal.with(|ixs| {
                        ixs.iter()
                            .any(|ix| !ix.removed && ix.status.contains("Indexed"))
                    });
                    (on_embed_step && any_indexed).then(|| view! {
                        <SearchSection
                            document_id=document_id
                            indexings=indexings_stored.get_value()
                            pipelines=pipelines_stored.get_value()
                        />
                    })
                }}
            </div>
        </div>
    }
}

#[component]
fn Stepper(
    active: ReadSignal<WorkflowStep>,
    set_active: WriteSignal<WorkflowStep>,
) -> impl IntoView {
    let steps = [
        WorkflowStep::Document,
        WorkflowStep::Chunk,
        WorkflowStep::EmbedIndex,
    ];

    view! {
        <div class="surface flex p-0 overflow-hidden">
            {steps.iter().enumerate().map(|(idx, step)| {
                let step = *step;
                let is_last = idx == steps.len() - 1;
                let is_active = move || active.get() == step;
                view! {
                    <button
                        type="button"
                        class=move || {
                            let base = "flex-1 flex items-center gap-3 px-4 py-3 text-left transition-colors";
                            if is_active() {
                                format!("{base} bg-[var(--color-surface-2)]")
                            } else {
                                format!("{base} hover:bg-[var(--color-surface-2)]")
                            }
                        }
                        on:click=move |_| set_active.set(step)
                    >
                        <span class=move || format!(
                            "inline-flex items-center justify-center w-7 h-7 rounded-full text-xs font-mono shrink-0 {}",
                            if is_active() {
                                "bg-[var(--color-accent)] text-[var(--color-page-bg)]"
                            } else {
                                "bg-[var(--color-surface-3)] text-[var(--color-text-muted)] border border-[var(--color-border)]"
                            }
                        )>
                            {step.ordinal()}
                        </span>
                        <div class="flex flex-col min-w-0">
                            <span class=move || format!(
                                "text-sm font-semibold {}",
                                if is_active() { "text-[var(--color-text)]" } else { "text-[var(--color-text-muted)]" }
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

#[component]
fn QuickIndexButton(source_ref: StoredValue<String>) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let run = move |_| {
        if busy.get_untracked() {
            return;
        }
        let slug = source_ref.get_value();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match start_indexing_with_defaults(slug).await {
                Ok(_) => {
                    toggle_drawer(true);
                    set_busy.set(false);
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    view! {
        <div class="flex flex-col items-end gap-1">
            <button
                type="button"
                class="btn btn-ghost"
                disabled=move || busy.get()
                title="Skip the wizard — chunk, embed, and index using the default pipeline & chunking config"
                on:click=run
            >
                {move || if busy.get() { "Running…" } else { "Quick: index with defaults" }}
            </button>
            {move || error.get().map(|e| view! {
                <div class="log-line-error text-xs">{e}</div>
            })}
        </div>
    }
}

fn initial_step(indexings: &[IndexingDto]) -> WorkflowStep {
    let live: Vec<&IndexingDto> = indexings.iter().filter(|ix| !ix.removed).collect();
    if live.is_empty() {
        return WorkflowStep::Document;
    }
    let has_progress = live.iter().any(|ix| ix.chunk_set_id.is_some());
    if has_progress {
        WorkflowStep::EmbedIndex
    } else {
        WorkflowStep::Chunk
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

    let most_advanced = live
        .iter()
        .copied()
        .max_by_key(|ix| milestone_rank(ix))
        .expect("live is non-empty");

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
        "BlogPost" => "Blog post",
        "Markdown" => "Markdown",
        "PlainText" => "Plain text",
        "WebPage" => "Web page",
        _ => "Document",
    }
}

#[component]
fn UnregisteredDocument(source_ref: String) -> impl IntoView {
    let source_ref_stored = StoredValue::new(source_ref.clone());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let import = move |_| {
        if busy.get_untracked() {
            return;
        }
        let slug = source_ref_stored.get_value();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match import_source_document(slug).await {
                Ok(_) => {
                    set_busy.set(false);
                }
                Err(e) => {
                    set_error.set(Some(format!("{e}")));
                    set_busy.set(false);
                }
            }
        });
    };

    view! {
        <div>
            <PageHeader
                title=source_ref.clone()
                eyebrow=format!("Documents / {source_ref}")
                subtitle="This document is available upstream but hasn't been imported yet. Import it to inspect chunks, run experiments, or index it.".to_string()
            />
            <Surface>
                <EmptyState
                    title="Not imported"
                    body="Importing fetches the upstream content and registers a versioned SourceDocument. After that you can run evaluations, chunk it with different strategies, and (optionally) index it.".to_string()
                    action=Box::new(move || view! {
                        <div class="flex flex-col items-start gap-2">
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=busy
                                on:click=import
                            >
                                {move || if busy.get() { "Importing…" } else { "Import from source" }}
                            </button>
                            {move || error.get().map(|e| view! {
                                <div class="log-line-error text-sm">{e}</div>
                            })}
                        </div>
                    }.into_any())
                />
            </Surface>
        </div>
    }
}
