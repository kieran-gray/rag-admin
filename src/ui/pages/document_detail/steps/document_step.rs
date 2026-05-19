use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;
use uuid::Uuid;

use crate::server_functions::source_document::{get_document_source, request_indexing};
use crate::shared::contracts::{
    ChunkingConfigurationDto, IndexingDto, MarkdownBlockDto, MarkdownBlockKindDto,
    PipelineConfigurationDto,
};
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::ConfigSelection;

#[component]
pub fn DocumentStep(
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

    let document_status: Signal<DocumentStatus> =
        Signal::derive(move || DocumentStatus::from(active_indexing.get().as_ref()));

    let auto_index = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(pipeline_id) = selection.pipeline_id.get() else {
            set_error.set(Some("Pick a pipeline first.".into()));
            return;
        };
        let default_chunking = chunking_configurations.with_value(|cs| {
            cs.iter()
                .find(|c| c.is_default)
                .or_else(|| cs.first())
                .map(|c| c.config)
        });
        let Some(config) = default_chunking else {
            set_error.set(Some("No chunking configuration available.".into()));
            return;
        };

        let slug = source_ref.get_value();
        set_busy.set(true);
        set_error.set(None);

        spawn_local(async move {
            match request_indexing(slug, pipeline_id, config, true).await {
                Ok(_) => set_busy.set(false),
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let auto_index_label = Signal::derive(move || {
        if busy.get() {
            return "Submitting…".to_string();
        }
        match document_status.get() {
            DocumentStatus::NotIndexed => "Auto-index with defaults".to_string(),
            DocumentStatus::InFlight => "Auto-index running…".to_string(),
            DocumentStatus::Indexed => "Re-run auto-index".to_string(),
            DocumentStatus::Failed => "Retry auto-index".to_string(),
            DocumentStatus::Partial => "Auto-index from scratch".to_string(),
        }
    });

    let auto_index_disabled = move || {
        busy.get()
            || selection.pipeline_id.get().is_none()
            || matches!(document_status.get(), DocumentStatus::InFlight)
    };

    view! {
        <div class="space-y-6">
            <Surface
                title="Pipeline".to_string()
                actions=Box::new(move || view! {
                    <Help title="What does auto-index do?".to_string()>
                        <p>
                            "Auto-index runs chunking, embedding, and indexing end-to-end using the default chunking strategy for the selected pipeline. Use this when you want a one-shot ingest and don't need to tune the chunking step."
                        </p>
                        <p class="mt-3">
                            "To pick a non-default chunking strategy or inspect intermediate output, step through the workflow with "
                            <span class="font-medium">"Continue to chunking →"</span>
                            " instead."
                        </p>
                    </Help>
                }.into_any())
            >
                <PipelineSelect selection=selection pipelines=pipelines />

                <div class="flex items-center justify-between gap-3 mt-4 pt-3 border-t border-[var(--color-border)]">
                    <DocumentStatusLine status=document_status />
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class="btn"
                            disabled=auto_index_disabled
                            on:click=auto_index
                        >
                            {move || auto_index_label.get()}
                        </button>
                        <button
                            type="button"
                            class="btn btn-primary"
                            on:click=move |_| on_advance.run(())
                        >
                            "Continue to chunking →"
                        </button>
                    </div>
                </div>
                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-sm mt-3">{e}</div>
                })}
            </Surface>

            <details class="surface collapsible-card" open>
                <summary class="collapsible-card-summary">
                    <span class="section-title">"Source inspector"</span>
                    <span class="collapsible-card-chevron">"▾"</span>
                </summary>
                <div class="collapsible-card-body">
                    <SourceInspector source_ref=source_ref.get_value() />
                </div>
            </details>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Chunk this document"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| on_advance.run(())
                >
                    "Continue to chunking →"
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
fn DocumentStatusLine(status: Signal<DocumentStatus>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 text-xs">
            {move || match status.get() {
                DocumentStatus::NotIndexed => view! {
                    <StatusPill label="Not indexed".to_string() kind=Status::Stale />
                }.into_any(),
                DocumentStatus::InFlight => view! {
                    <StatusPill label="Indexing…".to_string() kind=Status::Pending />
                }.into_any(),
                DocumentStatus::Indexed => view! {
                    <StatusPill label="Indexed".to_string() kind=Status::Ok />
                }.into_any(),
                DocumentStatus::Partial => view! {
                    <StatusPill label="Partial".to_string() kind=Status::Info />
                }.into_any(),
                DocumentStatus::Failed => view! {
                    <StatusPill label="Indexing failed".to_string() kind=Status::Fail />
                }.into_any(),
            }}
        </div>
    }
}

#[component]
pub fn SourceInspector(source_ref: String) -> impl IntoView {
    let source_ref_stored = StoredValue::new(source_ref);
    let body = Resource::new(
        move || source_ref_stored.get_value(),
        move |slug| async move { get_document_source(slug).await.map_err(|e| e.to_string()) },
    );

    let (show_raw, set_show_raw) = signal(false);

    let query = use_query_map();
    let ref_range = Memo::new(move |_| {
        query.with(|q| {
            let start = q.get("ref_start").and_then(|v| v.parse::<u32>().ok());
            let end = q.get("ref_end").and_then(|v| v.parse::<u32>().ok());
            match (start, end) {
                (Some(s), Some(e)) if e > s => Some((s, e)),
                _ => None,
            }
        })
    });

    let toggle = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        ev.prevent_default();
        set_show_raw.update(|v| *v = !*v);
    };

    view! {
        <Transition fallback=|| view! { <p class="muted text-sm">"Loading source…"</p> }>
            {move || body.get().map(|res| match res {
                Err(e) => view! {
                    <div class="log-line-error">{format!("Failed to load source: {e}")}</div>
                }.into_any(),
                Ok(None) => view! {
                    <EmptyState
                        title="No source on file"
                        body="This document hasn't been imported yet, so there is no markdown body to render.".to_string()
                    />
                }.into_any(),
                Ok(Some(doc)) => {
                    let version = doc.version;
                    let doc_stored = StoredValue::new(doc);
                    let body_view = move || {
                        let doc = doc_stored.get_value();
                        if show_raw.get() {
                            view! { <RawMarkdown source=doc.source /> }.into_any()
                        } else {
                            view! { <RenderedMarkdown blocks=doc.blocks ref_range=ref_range /> }.into_any()
                        }
                    };
                    view! {
                        <div class="space-y-3">
                            <div class="flex items-center justify-between text-xs muted">
                                <span>{format!("Document v{version}")}</span>
                                <button
                                    type="button"
                                    class="btn btn-sm"
                                    on:click=toggle
                                >
                                    {move || if show_raw.get() { "Show rendered" } else { "Show raw markdown" }}
                                </button>
                            </div>
                            {body_view}
                        </div>
                    }.into_any()
                }
            })}
        </Transition>
    }
}

#[component]
fn RawMarkdown(source: String) -> impl IntoView {
    view! {
        <pre class="md-raw">{source}</pre>
    }
}

#[component]
fn RenderedMarkdown(
    blocks: Vec<MarkdownBlockDto>,
    ref_range: Memo<Option<(u32, u32)>>,
) -> impl IntoView {
    let blocks = StoredValue::new(blocks);

    Effect::new(move |_| {
        if ref_range.get().is_some() {
            request_scroll_to_highlight();
        }
    });

    let rows = move || {
        let range = ref_range.get();
        let mut anchor_assigned = false;
        blocks
            .with_value(Clone::clone)
            .into_iter()
            .map(|block| {
                let highlighted =
                    range.is_some_and(|(s, e)| block.char_start < e && block.char_end > s);
                let is_anchor = highlighted && !anchor_assigned;
                if is_anchor {
                    anchor_assigned = true;
                }
                let mut class = format!("md-block md-{}", kind_class(block.kind));
                if highlighted {
                    class.push_str(" is-highlighted");
                }
                let id_attr = if is_anchor { Some("ref-anchor") } else { None };

                view! {
                    <div
                        class=class
                        id=id_attr
                        data-start=block.char_start.to_string()
                        data-end=block.char_end.to_string()
                        inner_html=block.html
                    ></div>
                }
            })
            .collect_view()
    };

    view! {
        <div class="md-document">
            {rows}
        </div>
    }
}

fn kind_class(kind: MarkdownBlockKindDto) -> &'static str {
    match kind {
        MarkdownBlockKindDto::Heading => "heading",
        MarkdownBlockKindDto::Paragraph => "paragraph",
        MarkdownBlockKindDto::List => "list",
        MarkdownBlockKindDto::CodeFence => "code",
        MarkdownBlockKindDto::BlockQuote => "blockquote",
        MarkdownBlockKindDto::Table => "table",
        MarkdownBlockKindDto::Html => "html",
        MarkdownBlockKindDto::ThematicBreak => "rule",
        MarkdownBlockKindDto::Other => "other",
    }
}

#[cfg(feature = "hydrate")]
fn request_scroll_to_highlight() {
    use wasm_bindgen::JsCast;
    leptos::task::spawn_local(async move {
        use gloo_timers::future::TimeoutFuture;

        TimeoutFuture::new(50).await;
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(el) = document.get_element_by_id("ref-anchor") {
                    if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
                        el.scroll_into_view_with_bool(true);
                    }
                }
            }
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn request_scroll_to_highlight() {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentStatus {
    NotIndexed,
    InFlight,
    Partial,
    Indexed,
    Failed,
}

impl DocumentStatus {
    fn from(indexing: Option<&IndexingDto>) -> Self {
        let Some(ix) = indexing else {
            return Self::NotIndexed;
        };
        if ix.status.contains("Failed") {
            return Self::Failed;
        }
        if ix.status.contains("Indexed") {
            return Self::Indexed;
        }
        if ix.chunk_set_id.is_none() || ix.status.contains("Indexing") {
            return Self::InFlight;
        }
        Self::Partial
    }
}
