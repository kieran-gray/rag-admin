use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::source_document::{get_chunks, request_indexing};
use crate::shared::contracts::{ChunkDto, ChunkingConfigurationDto, IndexingDto};
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::ConfigSelection;

#[component]
pub fn ChunkStep(
    selection: ConfigSelection,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    indexings: Signal<Vec<IndexingDto>>,
    source_ref: String,
    on_advance: Callback<()>,
) -> impl IntoView {
    let source_ref = StoredValue::new(source_ref);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let active_indexing: Signal<Option<IndexingDto>> = Signal::derive(move || {
        let pid = selection.index_profile_id.get()?;
        indexings
            .get()
            .into_iter()
            .find(|ix| ix.index_profile_id == pid && !ix.removed)
    });

    let chunk_status: Signal<ChunkStatus> =
        Signal::derive(move || ChunkStatus::from(active_indexing.get().as_ref()));

    let run_chunk = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(index_profile_id) = selection.index_profile_id.get() else {
            set_error.set(Some("Pick an index profile first.".into()));
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
            match request_indexing(slug, index_profile_id, config, false).await {
                Ok(_) => {
                    set_busy.set(false);
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let advance_disabled = move || !chunk_status.get().has_chunks();

    view! {
        <div class="space-y-6">
            <Surface
                title="Chunking configuration".to_string()
                actions=Box::new(move || view! {
                    <Help title="How chunking works".to_string()>
                        <p>
                            "The chunking step splits the document's markdown into smaller, retrievable pieces using the selected strategy. Once chunks exist you can preview them below before embedding."
                        </p>
                        <p class="mt-3">
                            "Different strategies trade off chunk size, overlap, and boundary detection. To compare strategies systematically, open the "
                            <span class="font-medium">"Evaluate"</span>
                            " workflow from the document header."
                        </p>
                    </Help>
                }.into_any())
            >
                <ChunkingSelect selection=selection chunking_configurations=chunking_configurations />

                <div class="flex items-center justify-between gap-3 mt-4 pt-3 border-t border-[var(--color-border)]">
                    <ChunkStatusLine status=chunk_status />
                    <button
                        type="button"
                        class="btn btn-primary"
                        disabled=move || busy.get()
                            || selection.index_profile_id.get().is_none()
                            || selection.chunking_id.get().is_none()
                        on:click=run_chunk
                    >
                        {move || if busy.get() {
                            "Submitting…".to_string()
                        } else if chunk_status.get().has_chunks() {
                            "Re-run chunking".to_string()
                        } else if matches!(chunk_status.get(), ChunkStatus::Failed) {
                            "Retry chunking".to_string()
                        } else {
                            "Run chunking".to_string()
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
                    <span class="collapsible-card-chevron">"▾"</span>
                </summary>
                <div class="collapsible-card-body">
                    <ChunkPreview active_indexing=active_indexing />
                </div>
            </details>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Embed the chunks"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=advance_disabled
                    on:click=move |_| on_advance.run(())
                >
                    {move || if chunk_status.get().has_chunks() {
                        "Continue to embedding →"
                    } else {
                        "Run chunking to continue"
                    }}
                </button>
            </div>
        </div>
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
                            title="Pick an index profile to begin"
                            body="Once you run chunking, the resulting chunks appear here.".to_string()
                        />
                    }.into_any();
                };
                if ix.chunk_set_id.is_none() {
                    return view! {
                        <EmptyState
                            title="No chunks yet"
                            body="Click \"Run chunking\" above to split the document using the selected configuration.".to_string()
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

#[component]
pub fn ChunkCard(chunk: ChunkDto) -> impl IntoView {
    let text_length = chunk.text.len();
    let heading = chunk.heading.clone();
    let sequence = chunk.sequence;
    let text = StoredValue::new(chunk.text);

    view! {
        <div class="surface-raised rounded p-3 flex flex-col gap-2">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="eyebrow">{format!("Chunk · seq {sequence:03}")}</div>
                    <div class="text-sm font-medium truncate">{heading}</div>
                </div>
                <span class="text-xs muted shrink-0">{format!("{text_length} chars")}</span>
            </div>
            <pre class="text-xs leading-relaxed whitespace-pre-wrap max-h-40 overflow-auto muted">
                {text.get_value()}
            </pre>
        </div>
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
