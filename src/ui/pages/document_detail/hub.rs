use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use super::map_phase::MapPhase;
use crate::server_functions::comprehension::list_document_maps;
use crate::server_functions::evaluation::{get_datasets_for_document, get_runs_for_document};
use crate::server_functions::source_document::{
    delete_document, get_document_detail_by_source_ref, get_document_source,
};
use crate::shared::contracts::{
    aggregate_type, DocumentMapSummaryDto, EvaluationDatasetSummaryDto, EvaluationRunSummaryDto,
    IndexingDto, MarkdownBlockDto, MarkdownBlockKindDto, SourceDocumentDetailDto,
};
use crate::ui::components::primitives::{
    ActionItem, ActionsMenu, ConfirmDialog, EmptyState, PageHeader, Status, StatusPill, Surface,
};
use crate::ui::pages::shared::{document_type_label, format_when, short_hash};
use crate::ui::state::event_bus::use_invalidator;

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

    view! {
        <div>
            <Transition fallback=|| view! {
                <p class="muted">"Loading document…"</p>
            }>
                {move || detail.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(None) => view! {
                        <NotFound source_ref=source_ref.get() />
                    }.into_any(),
                    Ok(Some(existing)) if existing.document.deleted => view! {
                        <Deleted source_ref=source_ref.get() />
                    }.into_any(),
                    Ok(Some(existing)) => view! {
                        <HubLayout detail=existing source_ref=source_ref.get() />
                    }.into_any(),
                })}
            </Transition>
        </div>
    }
}

#[component]
fn HubLayout(detail: SourceDocumentDetailDto, source_ref: String) -> impl IntoView {
    let document = detail.document.clone();
    let document_id = document.document_id;
    let document_version = document.latest_version;
    let document_type = document.document_type.clone();
    let source_ref_key = document.source_ref_key.clone();

    let header_eyebrow = format!(
        "Documents / {} / {}",
        document_type_label(&document.document_type),
        document.source_ref_key,
    );
    let header_subtitle = format!(
        "{} · v{} · {}",
        document_type_label(&document.document_type),
        document.latest_version,
        short_hash(&document.latest_content_hash),
    );

    let indexings = detail.indexings.clone();
    let indexings_stored = StoredValue::new(indexings.clone());
    let (status_kind, status_label) = overall_status(&indexings, document_version);

    let (confirm_delete_open, set_confirm_delete_open) = signal(false);
    let (deleting, set_deleting) = signal(false);
    let (delete_error, set_delete_error) = signal::<Option<String>>(None);

    let open_confirm_delete = Callback::new(move |_| {
        set_delete_error.set(None);
        set_confirm_delete_open.set(true);
    });
    let close_confirm_delete = Callback::new(move |_| set_confirm_delete_open.set(false));
    let confirm_delete = Callback::new(move |_| {
        if deleting.get_untracked() {
            return;
        }
        set_deleting.set(true);
        set_delete_error.set(None);
        spawn_local(async move {
            match delete_document(document_id).await {
                Ok(_) => {
                    set_deleting.set(false);
                    set_confirm_delete_open.set(false);
                }
                Err(e) => {
                    set_deleting.set(false);
                    set_confirm_delete_open.set(false);
                    set_delete_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    let actions = vec![ActionItem::new(
        "Delete document",
        Callback::new(move |_| open_confirm_delete.run(())),
    )
    .danger()];

    let _ = source_ref;
    let title = document.title.clone();

    view! {
        <div>
            <PageHeader
                title=title
                eyebrow=header_eyebrow
                subtitle=header_subtitle
                actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <StatusPill label=status_label.clone() kind=status_kind />
                        <ActionsMenu items=actions.clone() />
                    </div>
                }.into_any())
            />

            {move || delete_error.get().map(|e| view! {
                <div class="log-line-error text-sm mb-4">{e}</div>
            })}

            <ConfirmDialog
                open=confirm_delete_open
                title="Delete document".to_string()
                message="This document and all of its indexings will be removed. This cannot be undone.".to_string()
                confirm_label="Delete document".to_string()
                confirm_busy_label="Deleting…".to_string()
                busy=deleting
                danger=true
                on_confirm=confirm_delete
                on_close=close_confirm_delete
            />

            <div class="space-y-4">
                <IngestCard
                    indexings=indexings_stored
                    document_type=document_type.clone()
                    source_ref_key=source_ref_key.clone()
                />
                <EvaluateCard
                    document_id=document_id
                    document_version=document_version
                    document_type=document_type
                    source_ref_key=source_ref_key.clone()
                />
                <SourcePanel source_ref_key=source_ref_key />
            </div>
        </div>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)]
fn SourcePanel(source_ref_key: String) -> impl IntoView {
    let (loaded, set_loaded) = signal(false);
    let key_stored = StoredValue::new(source_ref_key);

    view! {
        <section class="surface p-4">
            <div class="flex items-center justify-between gap-3 mb-3">
                <div class="flex flex-col gap-0.5 min-w-0">
                    <span class="eyebrow">"Source"</span>
                    <span class="text-xs muted">
                        "The rendered markdown body. Hidden by default for large documents."
                    </span>
                </div>
                <button
                    type="button"
                    class=move || if loaded.get() { "btn btn-ghost btn-compact" } else { "btn btn-compact" }
                    on:click=move |_| set_loaded.update(|v| *v = !*v)
                >
                    {move || if loaded.get() { "Hide source" } else { "Show source" }}
                </button>
            </div>
            {move || loaded.get().then(|| view! {
                <SourceLoader source_ref_key=key_stored.get_value() />
            })}
        </section>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)]
fn SourceLoader(source_ref_key: String) -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::SOURCE_DOCUMENT]));
    let key_stored = StoredValue::new(source_ref_key);
    let source = Resource::new(
        move || (key_stored.get_value(), invalidator.get()),
        move |(slug, _)| async move { get_document_source(slug).await.map_err(|e| e.to_string()) },
    );

    view! {
        <Transition fallback=|| view! { <p class="text-xs muted">"Loading source…"</p> }>
            {move || source.get().map(|res| match res {
                Err(e) => view! {
                    <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                }.into_any(),
                Ok(None) => view! {
                    <p class="text-xs muted">"No source available."</p>
                }.into_any(),
                Ok(Some(markdown)) => view! {
                    <MarkdownBody blocks=markdown.blocks />
                }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn MarkdownBody(blocks: Vec<MarkdownBlockDto>) -> impl IntoView {
    view! {
        <div class="md-document">
            {blocks.into_iter().map(|block| view! {
                <div
                    class=format!("md-block md-{}", block_kind_class(block.kind))
                    inner_html=block.html
                ></div>
            }).collect_view()}
        </div>
    }
}

fn block_kind_class(kind: MarkdownBlockKindDto) -> &'static str {
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

#[component]
#[allow(clippy::needless_pass_by_value)]
fn IngestCard(
    indexings: StoredValue<Vec<IndexingDto>>,
    document_type: String,
    source_ref_key: String,
) -> impl IntoView {
    let workflow_href = format!(
        "/documents/{}/{}/ingest",
        document_type,
        urlencoding::encode(&source_ref_key),
    );

    let ingest = indexings.with_value(|v| summarize_ingest(v));
    let is_indexed = matches!(ingest.primary, IngestPrimary::Inspect);
    let inspect_href = format!("{workflow_href}?step={}", ingest.primary_step_query());
    let primary_label = ingest.primary_label();

    view! {
        <section class="surface p-4">
            <header class="flex items-center justify-between gap-3 mb-3">
                <div class="flex flex-col gap-0.5 min-w-0">
                    <span class="eyebrow">"Ingest"</span>
                    <span class="text-xs muted">"Chunk, embed, and index the document for retrieval."</span>
                </div>
                <A href=workflow_href.clone() attr:class="btn btn-ghost btn-compact">
                    "Open workflow →"
                </A>
            </header>

            <WorkflowStrip
                nodes=vec![
                    StepNode { label: "Chunk", detail: ingest.chunk_detail, state: ingest.chunk_state, href: format!("{workflow_href}?step=chunk") },
                    StepNode { label: "Embed", detail: ingest.embed_detail, state: ingest.embed_state, href: format!("{workflow_href}?step=embed") },
                    StepNode { label: "Index", detail: ingest.index_detail, state: ingest.index_state, href: format!("{workflow_href}?step=index") },
                ]
            />

            <div class="mt-3 pt-3 border-t border-[var(--color-border)] flex items-center justify-end gap-2">
                {if is_indexed {
                    Either::Left(view! {
                        <A href=inspect_href attr:class="btn btn-ghost btn-compact">
                            "Inspect ingest →"
                        </A>
                        <A href="/playground/chat" attr:class="btn btn-primary btn-compact">
                            "Open in playground →"
                        </A>
                    })
                } else {
                    Either::Right(view! {
                        <A href=inspect_href attr:class="btn btn-primary btn-compact">
                            {primary_label}
                        </A>
                    })
                }}
            </div>
        </section>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)]
fn EvaluateCard(
    document_id: Uuid,
    document_version: u32,
    document_type: String,
    source_ref_key: String,
) -> impl IntoView {
    let map_invalidator = use_invalidator(|e| e.from_any(&["document_map"]));
    let dataset_invalidator =
        use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_DATASET]));
    let run_invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));

    let maps = Resource::new(
        move || (document_id, map_invalidator.get()),
        move |(id, _)| async move { list_document_maps(id).await.unwrap_or_default() },
    );
    let datasets = Resource::new(
        move || (document_id, dataset_invalidator.get()),
        move |(id, _)| async move { get_datasets_for_document(id).await.unwrap_or_default() },
    );
    let runs = Resource::new(
        move || (document_id, run_invalidator.get()),
        move |(id, _)| async move { get_runs_for_document(id).await.unwrap_or_default() },
    );

    let evaluate_href = format!("/evaluate/by-id/{document_id}");
    let map_href = format!(
        "/documents/{}/{}/map",
        document_type,
        urlencoding::encode(&source_ref_key),
    );

    view! {
        <section class="surface p-4">
            <header class="flex items-center justify-between gap-3 mb-3">
                <div class="flex flex-col gap-0.5 min-w-0">
                    <span class="eyebrow">"Evaluate"</span>
                    <span class="text-xs muted">"Map the document, generate questions, run evaluations."</span>
                </div>
                <A href=evaluate_href.clone() attr:class="btn btn-ghost btn-compact">
                    "Open workflow →"
                </A>
            </header>

            <Transition fallback=|| view! {
                <p class="text-xs muted">"Loading evaluation state…"</p>
            }>
                {
                    let evaluate_href = evaluate_href.clone();
                    let map_href = map_href.clone();
                    move || {
                        let evaluate_href = evaluate_href.clone();
                        let map_href = map_href.clone();
                        let map_list = maps.get();
                        let dataset_list = datasets.get();
                        let run_list = runs.get();
                        match (map_list, dataset_list, run_list) {
                            (Some(map_list), Some(dataset_list), Some(run_list)) => {
                                let current_map = map_list.into_iter()
                                    .find(|m| m.document_version == document_version);
                                let eval = summarize_evaluate(current_map.as_ref(), &dataset_list, &run_list);
                                let primary_href = match eval.primary_action {
                                    EvalPrimary::BuildMap => map_href.clone(),
                                    EvalPrimary::Continue => evaluate_href.clone(),
                                };
                                let primary_label = eval.primary_label();
                                let recent: Vec<EvaluationRunSummaryDto> =
                                    run_list.iter().take(3).cloned().collect();
                                Either::Left(view! {
                                    <WorkflowStrip
                                        nodes=vec![
                                            StepNode { label: "Map", detail: eval.map_detail, state: eval.map_state, href: map_href },
                                            StepNode { label: "Dataset", detail: eval.dataset_detail, state: eval.dataset_state, href: format!("{evaluate_href}?step=dataset") },
                                            StepNode { label: "Run", detail: eval.run_detail, state: eval.run_state, href: format!("{evaluate_href}?step=run") },
                                        ]
                                    />

                                    {(!recent.is_empty()).then(|| view! {
                                        <div class="mt-3 pt-3 border-t border-[var(--color-border)]">
                                            <div class="eyebrow mb-2">"Recent runs"</div>
                                            <ul class="flex flex-col gap-1">
                                                {recent.into_iter().map(|r| view! {
                                                    <RecentRunRow run=r />
                                                }).collect_view()}
                                            </ul>
                                        </div>
                                    })}

                                    <div class="mt-3 pt-3 border-t border-[var(--color-border)] flex items-center justify-end">
                                        <A href=primary_href attr:class="btn btn-primary btn-compact">
                                            {primary_label}
                                        </A>
                                    </div>
                                })
                            }
                            _ => Either::Right(view! {
                                <p class="text-xs muted">"Loading evaluation state…"</p>
                            }),
                        }
                    }
                }
            </Transition>
        </section>
    }
}

#[component]
fn RecentRunRow(run: EvaluationRunSummaryDto) -> impl IntoView {
    let run_id = run.run_id;
    let href = format!("/workflows/runs/{run_id}");
    let when = format_when(&run.created_at);
    let label = run_short_label(run_id);
    let (status_kind, status_label) = run_status_pill(&run.status);
    let variants_line = if run.variant_count == 1 {
        "1 variant".to_string()
    } else {
        format!("{} variants", run.variant_count)
    };

    view! {
        <li>
            <A href=href attr:class="flex items-center gap-3 px-2 py-1.5 rounded hover:bg-[var(--color-surface-2)] text-sm">
                <span class="font-mono text-xs muted shrink-0">{label}</span>
                <StatusPill label=status_label kind=status_kind />
                <span class="font-mono text-xs muted truncate flex-1">{variants_line}</span>
                <span class="text-xs muted shrink-0">{when}</span>
            </A>
        </li>
    }
}

fn run_status_pill(status: &str) -> (Status, String) {
    match status {
        "completed" => (Status::Ok, "Completed".to_string()),
        "failed" => (Status::Fail, "Failed".to_string()),
        "running" => (Status::Pending, "Running".to_string()),
        "pending" => (Status::Pending, "Pending".to_string()),
        other => (Status::Neutral, other.to_string()),
    }
}

#[derive(Clone)]
struct StepNode {
    label: &'static str,
    detail: String,
    state: NodeState,
    href: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeState {
    Empty,
    Active,
    Done,
    Failed,
}

#[component]
fn WorkflowStrip(nodes: Vec<StepNode>) -> impl IntoView {
    let total = nodes.len();
    view! {
        <div class="flex items-stretch gap-2 flex-wrap">
            {nodes.into_iter().enumerate().map(|(idx, node)| {
                let is_last = idx == total - 1;
                let (icon, ring) = match node.state {
                    NodeState::Done => (
                        "✓",
                        "bg-[var(--color-accent-soft)] text-[var(--color-accent)] border-[var(--color-accent)]",
                    ),
                    NodeState::Active => (
                        "●",
                        "bg-[var(--status-info)] text-[var(--color-page-bg)] border-[var(--status-info)] animate-pulse",
                    ),
                    NodeState::Failed => (
                        "✗",
                        "bg-[var(--color-page-bg)] text-[var(--status-fail)] border-[var(--status-fail)]",
                    ),
                    NodeState::Empty => (
                        "○",
                        "bg-transparent text-[var(--color-text-faint)] border-[var(--color-border)]",
                    ),
                };
                view! {
                    <div class="flex items-center gap-2 min-w-0">
                        <A href=node.href attr:class="flex items-center gap-2 min-w-0 px-2 py-1 rounded hover:bg-[var(--color-surface-2)]">
                            <span class=format!("inline-flex items-center justify-center w-6 h-6 rounded-full border text-xs {ring}")>
                                {icon}
                            </span>
                            <div class="flex flex-col leading-tight min-w-0">
                                <span class="text-xs font-semibold text-[var(--color-text)]">{node.label}</span>
                                {(!node.detail.is_empty()).then(|| view! {
                                    <span class="text-[11px] muted font-mono truncate">{node.detail.clone()}</span>
                                })}
                            </div>
                        </A>
                        {(!is_last).then(|| view! { <span class="faint px-1">"›"</span> })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

struct IngestSummary {
    chunk_state: NodeState,
    chunk_detail: String,
    embed_state: NodeState,
    embed_detail: String,
    index_state: NodeState,
    index_detail: String,
    primary: IngestPrimary,
}

enum IngestPrimary {
    StartChunk,
    ContinueEmbed,
    ContinueIndex,
    Inspect,
}

impl IngestSummary {
    fn primary_label(&self) -> &'static str {
        match self.primary {
            IngestPrimary::StartChunk => "Start chunking →",
            IngestPrimary::ContinueEmbed => "Continue: embed →",
            IngestPrimary::ContinueIndex => "Continue: index →",
            IngestPrimary::Inspect => "Inspect ingest →",
        }
    }

    fn primary_step_query(&self) -> &'static str {
        match self.primary {
            IngestPrimary::StartChunk => "chunk",
            IngestPrimary::ContinueEmbed => "embed",
            IngestPrimary::ContinueIndex | IngestPrimary::Inspect => "index",
        }
    }
}

fn summarize_ingest(indexings: &[IndexingDto]) -> IngestSummary {
    let live: Vec<&IndexingDto> = indexings.iter().filter(|ix| !ix.removed).collect();
    let has_chunk = live.iter().any(|ix| ix.chunk_set_id.is_some());
    let has_embed = live.iter().any(|ix| ix.embedding_set_id.is_some());
    let is_indexed = live.iter().any(|ix| ix.status.contains("Indexed"));

    let chunk_state = if has_chunk {
        NodeState::Done
    } else {
        NodeState::Empty
    };
    let embed_state = if has_embed {
        NodeState::Done
    } else {
        NodeState::Empty
    };
    let index_state = if is_indexed {
        NodeState::Done
    } else {
        NodeState::Empty
    };

    let primary = if !has_chunk {
        IngestPrimary::StartChunk
    } else if !has_embed {
        IngestPrimary::ContinueEmbed
    } else if !is_indexed {
        IngestPrimary::ContinueIndex
    } else {
        IngestPrimary::Inspect
    };

    let indexing_count = live.len();
    let chunk_detail = if has_chunk {
        if indexing_count > 1 {
            format!("{indexing_count} configs")
        } else {
            "ready".to_string()
        }
    } else {
        String::new()
    };
    let embed_detail = if has_embed {
        "ready".to_string()
    } else {
        String::new()
    };
    let index_detail = if is_indexed {
        let n = live
            .iter()
            .filter(|ix| ix.status.contains("Indexed"))
            .count();
        if n > 1 {
            format!("{n} indexed")
        } else {
            "indexed".to_string()
        }
    } else {
        String::new()
    };

    IngestSummary {
        chunk_state,
        chunk_detail,
        embed_state,
        embed_detail,
        index_state,
        index_detail,
        primary,
    }
}

struct EvalSummary {
    map_state: NodeState,
    map_detail: String,
    dataset_state: NodeState,
    dataset_detail: String,
    run_state: NodeState,
    run_detail: String,
    primary_action: EvalPrimary,
    primary_label_text: String,
}

enum EvalPrimary {
    BuildMap,
    Continue,
}

impl EvalSummary {
    fn primary_label(&self) -> String {
        self.primary_label_text.clone()
    }
}

fn summarize_evaluate(
    map: Option<&DocumentMapSummaryDto>,
    datasets: &[EvaluationDatasetSummaryDto],
    runs: &[EvaluationRunSummaryDto],
) -> EvalSummary {
    let (map_state, map_detail) = match map {
        Some(m) => {
            let phase = MapPhase::from_status(&m.status);
            let state = if phase.is_ready() {
                NodeState::Done
            } else if phase.is_failed() {
                NodeState::Failed
            } else {
                NodeState::Active
            };
            let detail = if phase.is_ready() {
                format!(
                    "{}i · {}t · {}o",
                    m.insights_synthesized, m.threads_synthesized, m.observations_extracted,
                )
            } else if phase.is_failed() {
                "failed".to_string()
            } else {
                "building".to_string()
            };
            (state, detail)
        }
        None => (NodeState::Empty, String::new()),
    };

    let dataset_count = datasets.len();
    let (dataset_state, dataset_detail) = if dataset_count == 0 {
        (NodeState::Empty, String::new())
    } else if dataset_count == 1 {
        (NodeState::Done, "1 dataset".to_string())
    } else {
        (NodeState::Done, format!("{dataset_count} datasets"))
    };

    let run_count = runs.len();
    let (run_state, run_detail) = if run_count == 0 {
        (NodeState::Empty, String::new())
    } else if run_count == 1 {
        (NodeState::Done, "1 run".to_string())
    } else {
        (NodeState::Done, format!("{run_count} runs"))
    };

    let map_ready = map.is_some_and(|m| MapPhase::from_status(&m.status).is_ready());
    let (primary_action, primary_label_text) = if !map_ready {
        (EvalPrimary::BuildMap, "Build map →".to_string())
    } else if dataset_count == 0 {
        (EvalPrimary::Continue, "Generate dataset →".to_string())
    } else if run_count == 0 {
        (EvalPrimary::Continue, "Launch first run →".to_string())
    } else {
        (EvalPrimary::Continue, "Launch new run →".to_string())
    };

    EvalSummary {
        map_state,
        map_detail,
        dataset_state,
        dataset_detail,
        run_state,
        run_detail,
        primary_action,
        primary_label_text,
    }
}

fn overall_status(indexings: &[IndexingDto], version: u32) -> (Status, String) {
    use crate::ui::pages::shared::indexing_summary;
    let summary = indexing_summary(indexings, Some(version));
    (summary.status, summary.label)
}

fn run_short_label(id: Uuid) -> String {
    let s = id.to_string();
    format!("run-{}", s.chars().take(6).collect::<String>())
}

#[component]
fn NotFound(source_ref: String) -> impl IntoView {
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

#[component]
fn Deleted(source_ref: String) -> impl IntoView {
    view! {
        <div>
            <PageHeader
                title=source_ref.clone()
                eyebrow=format!("Documents / {source_ref}")
                subtitle="This document has been deleted.".to_string()
            />
            <Surface>
                <EmptyState
                    title="Deleted"
                    body="This document was deleted. Re-import it from the Documents page to bring it back.".to_string()
                />
            </Surface>
        </div>
    }
}
