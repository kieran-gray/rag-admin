use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::contracts::{ChunkDto, ChunkingConfigurationDto, IndexingDto, PipelineConfigurationDto};
use crate::server_functions::source_document::{get_chunks, request_indexing};
use crate::ui::components::activity::toggle_drawer;
use crate::ui::components::primitives::{EmptyState, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::ConfigSelection;
use crate::ui::pages::document_detail::tabs::chunks::chunk_card::ChunkCard;

#[component]
pub fn ChunkStep(
    selection: ConfigSelection,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    indexings: Signal<Vec<IndexingDto>>,
    source_ref: String,
    on_advance: Callback<()>,
) -> impl IntoView {
    let source_ref = StoredValue::new(source_ref);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let active_indexing: Signal<Option<IndexingDto>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        indexings
            .get()
            .into_iter()
            .find(|ix| ix.pipeline_configuration_id == pid && !ix.removed)
    });

    let chunk_status: Signal<ChunkStatus> =
        Signal::derive(move || ChunkStatus::from(active_indexing.get().as_ref()));

    let run_chunk = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(pipeline_id) = selection.pipeline_id.get() else {
            set_error.set(Some("Pick a pipeline first.".into()));
            return;
        };
        let Some(chunking_id) = selection.chunking_id.get() else {
            set_error.set(Some("Pick a chunking configuration first.".into()));
            return;
        };
        let chunking_config = chunking_configurations.with_value(|cs| {
            cs.iter()
                .find(|c| c.chunking_configuration_id == chunking_id)
                .map(|c| c.config)
        });
        let Some(config) = chunking_config else {
            set_error.set(Some("Selected chunking configuration is missing.".into()));
            return;
        };

        let slug = source_ref.get_value();
        set_busy.set(true);
        set_error.set(None);

        spawn_local(async move {
            match request_indexing(slug, pipeline_id, config, false).await {
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
        <div class="space-y-6">
            <Surface title="Pipeline & chunking configuration".to_string()>
                <p class="text-sm muted mb-4">
                    "Pick the pipeline (which embedding & vector store to use) and a chunking strategy, then click "
                    <span class="text-text font-medium">"Chunk"</span>
                    " to split the document. You'll preview the resulting chunks before embedding."
                </p>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <PipelineSelect selection=selection pipelines=pipelines />
                    <ChunkingSelect selection=selection chunking_configurations=chunking_configurations />
                </div>

                <div class="flex items-center justify-between gap-3 mt-4 pt-3 border-t border-[var(--color-border)]">
                    <ChunkStatusLine status=chunk_status />
                    <button
                        type="button"
                        class="btn btn-primary"
                        disabled=move || busy.get()
                            || selection.pipeline_id.get().is_none()
                            || selection.chunking_id.get().is_none()
                        on:click=run_chunk
                    >
                        {move || if busy.get() {
                            "Submitting…".to_string()
                        } else {
                            match chunk_status.get() {
                                ChunkStatus::None => "Chunk this document".to_string(),
                                ChunkStatus::InFlight => "Chunking…".to_string(),
                                ChunkStatus::Failed => "Retry chunking".to_string(),
                                _ => "Re-chunk".to_string(),
                            }
                        }}
                    </button>
                </div>
                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-sm mt-3">{e}</div>
                })}
            </Surface>

            <details class="surface collapsible-card" open>
                <summary class="collapsible-card-summary">
                    <span class="section-title">"Chunk inspector"</span>
                    <span class="muted text-xs collapsible-card-hint">
                        <span class="when-open">"Click to collapse"</span>
                        <span class="when-closed">"Click to expand"</span>
                        <span class="collapsible-card-chevron">"▾"</span>
                    </span>
                </summary>
                <div class="collapsible-card-body">
                    <ChunkPreview active_indexing=active_indexing />
                </div>
            </details>

            <div class="flex justify-end pt-2">
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=move || !chunk_status.get().has_chunks()
                    on:click=move |_| on_advance.run(())
                >
                    {move || if chunk_status.get().has_chunks() {
                        "Proceed to embedding →"
                    } else {
                        "Chunk first to continue"
                    }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn PipelineSelect(
    selection: ConfigSelection,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 text-sm">
            <span class="eyebrow">"Pipeline"</span>
            <select
                class="input"
                on:change=move |ev| {
                    selection.pipeline_id.set(Uuid::parse_str(&event_target_value(&ev)).ok());
                }
            >
                {move || pipelines.with_value(|ps| {
                    ps.iter().map(|p| {
                        let id = p.pipeline_configuration_id;
                        let suffix = if p.is_default { " · default" } else { "" };
                        let name = p.name.clone();
                        let embedding = p.embedding_model_name.clone().unwrap_or_default();
                        let label = if embedding.is_empty() {
                            format!("{name}{suffix}")
                        } else {
                            format!("{name}{suffix} · {embedding}")
                        };
                        let selected = selection.pipeline_id.get() == Some(id);
                        view! { <option value=id.to_string() selected=selected>{label}</option> }
                    }).collect_view()
                })}
            </select>
        </label>
    }
}

#[component]
fn ChunkingSelect(
    selection: ConfigSelection,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 text-sm">
            <span class="eyebrow">"Chunking configuration"</span>
            <select
                class="input"
                on:change=move |ev| {
                    selection.chunking_id.set(Uuid::parse_str(&event_target_value(&ev)).ok());
                }
            >
                {move || chunking_configurations.with_value(|cs| {
                    cs.iter().map(|c| {
                        let id = c.chunking_configuration_id;
                        let suffix = if c.is_default { " · default" } else { "" };
                        let name = c.name.clone();
                        let descriptor = c.config.describe();
                        let selected = selection.chunking_id.get() == Some(id);
                        view! {
                            <option value=id.to_string() selected=selected>
                                {format!("{name}{suffix} · {descriptor}")}
                            </option>
                        }
                    }).collect_view()
                })}
            </select>
        </label>
    }
}

#[component]
fn ChunkStatusLine(status: Signal<ChunkStatus>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 text-xs">
            {move || match status.get() {
                ChunkStatus::None => view! {
                    <StatusPill label="Not chunked".to_string() kind=Status::Stale />
                }.into_any(),
                ChunkStatus::InFlight => view! {
                    <StatusPill label="Chunking…".to_string() kind=Status::Pending />
                }.into_any(),
                ChunkStatus::Ready => view! {
                    <StatusPill label="Chunked".to_string() kind=Status::Ok />
                }.into_any(),
                ChunkStatus::Embedded => view! {
                    <StatusPill label="Embedded".to_string() kind=Status::Ok />
                }.into_any(),
                ChunkStatus::Indexed => view! {
                    <StatusPill label="Indexed".to_string() kind=Status::Ok />
                }.into_any(),
                ChunkStatus::Failed => view! {
                    <StatusPill label="Chunking failed".to_string() kind=Status::Fail />
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn ChunkPreview(active_indexing: Signal<Option<IndexingDto>>) -> impl IntoView {
    let chunks = Resource::new(
        move || active_indexing.get().and_then(|ix| ix.chunk_set_id),
        move |cid| async move {
            match cid {
                Some(id) => get_chunks(id).await.map_err(|e| e.to_string()),
                None => Ok(Vec::<ChunkDto>::new()),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted text-sm">"Loading chunks…"</p> }>
            {move || {
                let Some(ix) = active_indexing.get() else {
                    return view! {
                        <EmptyState
                            title="Pick a pipeline to begin"
                            body="Once you click Chunk, the resulting chunks appear here.".to_string()
                        />
                    }.into_any();
                };
                if ix.chunk_set_id.is_none() {
                    return view! {
                        <EmptyState
                            title="No chunks yet"
                            body="Click \"Chunk this document\" above to split it using the selected configuration.".to_string()
                        />
                    }.into_any();
                }
                chunks.get().map(|res| match res {
                    Err(e) => view! {
                        <div class="log-line-error">{format!("Failed to load chunks: {e}")}</div>
                    }.into_any(),
                    Ok(list) if list.is_empty() => view! {
                        <EmptyState
                            title="Empty chunk set"
                            body="The chunker returned no chunks. Check the chunking configuration and try again.".to_string()
                        />
                    }.into_any(),
                    Ok(list) => {
                        let total = list.len();
                        view! {
                            <div class="space-y-3">
                                <div class="text-xs muted">{format!("{total} chunks")}</div>
                                <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                                    {list.into_iter().map(|c| view! { <ChunkCard chunk=c /> }).collect_view()}
                                </div>
                            </div>
                        }.into_any()
                    }
                }).unwrap_or_else(|| view! { <p class="muted text-sm">"Loading chunks…"</p> }.into_any())
            }}
        </Transition>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStatus {
    None,
    InFlight,
    Ready,
    Embedded,
    Indexed,
    Failed,
}

impl ChunkStatus {
    fn from(indexing: Option<&IndexingDto>) -> Self {
        let Some(ix) = indexing else {
            return Self::None;
        };
        if ix.status.contains("Failed") {
            return Self::Failed;
        }
        match ix.chunk_set_id {
            None => Self::InFlight,
            Some(_) => {
                if ix.status.contains("Indexed") {
                    Self::Indexed
                } else if ix.status.contains("Embedding") {
                    Self::Embedded
                } else {
                    Self::Ready
                }
            }
        }
    }

    pub fn has_chunks(self) -> bool {
        matches!(self, Self::Ready | Self::Embedded | Self::Indexed)
    }
}
