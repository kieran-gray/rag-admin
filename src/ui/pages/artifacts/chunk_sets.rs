use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::query_signal;

use crate::server_functions::chunk_set::{
    delete_chunk_set, gc_chunk_sets, get_chunk_sets_page, set_chunk_set_pinned,
};
use crate::shared::contracts::{
    aggregate_type, ChunkSetListPageDto, ChunkSetListQueryDto, ChunkSetStatusFilterDto,
    ChunkSetSummaryDto, DeleteChunkSetRequestDto, GcChunkSetsRequestDto,
    SetChunkSetPinnedRequestDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    use_cursor_pagination, EmptyFilterState, EmptyState, FilterChip, InlineStatus,
    InlineStatusMessage, PageHeader, PaginationBar, PaginationSummary, SkeletonColumn,
    SkeletonRows, Surface, TitleCell,
};
use crate::ui::pages::shared::format_when;

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FilterState {
    statuses: Vec<ChunkSetStatusFilterDto>,
}

impl FilterState {
    fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    fn to_query(&self, cursor: Option<String>) -> ChunkSetListQueryDto {
        ChunkSetListQueryDto {
            cursor,
            limit: PAGE_SIZE,
            statuses: self.statuses.clone(),
        }
    }
}

#[component]
pub fn ChunkSetsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::INDEXING, aggregate_type::EVALUATION_RUN])
    });

    let (statuses_query, set_statuses_query) = query_signal::<String>("status");

    let filter_state = Memo::new(move |_| FilterState {
        statuses: statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(ChunkSetStatusFilterDto::from_slug)
                    .collect()
            })
            .unwrap_or_default(),
    });

    let pagination = use_cursor_pagination();
    let pagination_reset = pagination.reset;

    Effect::new(move |prev: Option<FilterState>| {
        let current = filter_state.get();
        if let Some(p) = prev.as_ref() {
            if p != &current {
                pagination_reset.run(());
            }
        }
        current
    });

    let current_cursor = pagination.current_cursor;
    let (refresh, set_refresh) = signal(0u32);
    let page = Resource::new(
        move || {
            (
                filter_state.get(),
                current_cursor.get(),
                invalidator.get(),
                refresh.get(),
            )
        },
        |(state, cursor, _, _)| async move { get_chunk_sets_page(state.to_query(cursor)).await },
    );

    let bump = move || set_refresh.update(|n| *n = n.wrapping_add(1));

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (gc_days, set_gc_days) = signal::<u32>(7);

    let toggle_status = Callback::<ChunkSetStatusFilterDto>::new(move |status| {
        let mut current = filter_state.get_untracked().statuses;
        if let Some(pos) = current.iter().position(|s| s == &status) {
            current.remove(pos);
        } else {
            current.push(status);
        }
        set_statuses_query.set((!current.is_empty()).then(|| {
            current
                .iter()
                .map(|s| s.slug())
                .collect::<Vec<_>>()
                .join(",")
        }));
    });

    let clear_filters = move |_| set_statuses_query.set(None);

    let push_cursor = pagination.push_cursor;
    let on_next = Callback::<()>::new(move |_| {
        page.with(|res| {
            if let Some(Ok(p)) = res.as_ref() {
                if let Some(next) = p.next_cursor.clone() {
                    push_cursor.run(next);
                }
            }
        });
    });

    let summary: Signal<Option<PaginationSummary>> = Signal::derive(move || {
        page.get().and_then(Result::ok).map(|p| PaginationSummary {
            shown: p.items.len(),
            total_matching: p.total_matching,
            total_all: p.total_all,
            has_next: p.next_cursor.is_some(),
        })
    });

    let on_pin = Callback::new(move |(id, pinned): (uuid::Uuid, bool)| {
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match set_chunk_set_pinned(SetChunkSetPinnedRequestDto {
                chunk_set_id: id,
                pinned,
            })
            .await
            {
                Ok(_) => {
                    set_status.set(Some(InlineStatusMessage::ok(if pinned {
                        "Pinned"
                    } else {
                        "Unpinned"
                    })));
                    bump();
                }
                Err(e) => set_status.set(Some(InlineStatusMessage::err(e.to_string()))),
            }
            set_busy.set(false);
        });
    });

    let on_delete = Callback::new(move |id: uuid::Uuid| {
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match delete_chunk_set(DeleteChunkSetRequestDto { chunk_set_id: id }).await {
                Ok(_) => {
                    set_status.set(Some(InlineStatusMessage::ok("Chunk set deleted")));
                    bump();
                }
                Err(e) => set_status.set(Some(InlineStatusMessage::err(e.to_string()))),
            }
            set_busy.set(false);
        });
    });

    let on_gc = move |_| {
        let days = gc_days.get_untracked();
        let secs = (days as u64).saturating_mul(86_400);
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match gc_chunk_sets(GcChunkSetsRequestDto {
                older_than_seconds: secs,
            })
            .await
            {
                Ok(resp) => {
                    set_status.set(Some(InlineStatusMessage::ok(format!(
                        "Deleted {} unused chunk set{}",
                        resp.deleted,
                        if resp.deleted == 1 { "" } else { "s" },
                    ))));
                    bump();
                }
                Err(e) => set_status.set(Some(InlineStatusMessage::err(e.to_string()))),
            }
            set_busy.set(false);
        });
    };

    view! {
        <div>
            <PageHeader
                title="Chunk sets"
                subtitle="Cached chunkings of documents, reused across indexing and evaluation runs. Pin to protect from cleanup; in-use chunk sets cannot be deleted directly.".to_string()
            />

            <InlineStatus status=status />

            <Surface title="Clean up".to_string() class="mb-2">
                <div class="flex items-center gap-3">
                    <label class="text-sm muted">"Delete unused chunk sets older than"</label>
                    <input
                        type="number"
                        min="0"
                        class="input w-20"
                        prop:value=move || gc_days.get().to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                set_gc_days.set(v);
                            }
                        }
                    />
                    <span class="text-sm muted">"days"</span>
                    <button
                        type="button"
                        class="btn btn-primary text-nowrap"
                        prop:disabled=move || busy.get()
                        on:click=on_gc
                    >
                        "Run cleanup"
                    </button>
                </div>
                <p class="text-xs muted mt-2">
                    "Skips pinned chunk sets and any referenced by an active indexing or evaluation run."
                </p>
            </Surface>

            <Surface flush=true>
                <FilterBar
                    page=page
                    filter_state=filter_state
                    toggle_status=toggle_status
                />
                <ActiveFilters
                    filter_state=filter_state
                    page=page
                    toggle_status=toggle_status
                    on_clear=Callback::new(clear_filters)
                />

                <Transition fallback=move || view! { <SkeletonChunkSetsTable /> }>
                    {move || page.get().map(|res| match res {
                        Err(e) => Either::Left(view! {
                            <div class="p-6 log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        }),
                        Ok(page_data) => Either::Right({
                            if page_data.items.is_empty() && page_data.total_all == 0 {
                                view! {
                                    <div class="p-6">
                                        <EmptyState
                                            title="No chunk sets"
                                            body="Chunk sets are created when you index a document or run an evaluation. They are reused across runs that share the same chunking config.".to_string()
                                        />
                                    </div>
                                }.into_any()
                            } else if page_data.items.is_empty() {
                                view! {
                                    <EmptyFilterState
                                        title="No chunk sets match these filters".to_string()
                                        body="Try removing a filter.".to_string()
                                        on_clear=Callback::new(clear_filters)
                                    />
                                }.into_any()
                            } else {
                                view! {
                                    <ChunkSetsTable
                                        items=page_data.items.clone()
                                        busy=busy
                                        on_pin=on_pin
                                        on_delete=on_delete
                                    />
                                }.into_any()
                            }
                        })
                    })}
                </Transition>

                <Suspense>
                    <PaginationBar
                        summary=summary
                        controls=pagination
                        on_next=on_next
                    />
                </Suspense>
            </Surface>
        </div>
    }
}

#[component]
fn FilterBar(
    page: Resource<Result<ChunkSetListPageDto, ServerFnError>>,
    filter_state: Memo<FilterState>,
    toggle_status: Callback<ChunkSetStatusFilterDto>,
) -> impl IntoView {
    let all_statuses = [
        ChunkSetStatusFilterDto::Pinned,
        ChunkSetStatusFilterDto::Indexed,
        ChunkSetStatusFilterDto::UsedByEval,
        ChunkSetStatusFilterDto::Unused,
    ];

    view! {
        <div class="list-page-toolbar">
            <div class="list-page-filter-group">
                <span class="list-page-filter-label">"Status"</span>
                <Transition fallback=|| view! { <span class="faint text-xs">"…"</span> }>
                    {move || page.get().and_then(Result::ok).map(move |p| {
                        all_statuses.iter().map(|status| {
                            let status = *status;
                            let count = p.status_counts.iter()
                                .find(|f| f.status == status)
                                .map(|f| f.count)
                                .unwrap_or(0);
                            let is_active = filter_state.with(|s| s.statuses.contains(&status));
                            let color = status_color(status);
                            if count == 0 && !is_active {
                                return ().into_any();
                            }
                            view! {
                                <FilterChip
                                    label=status.label().to_string()
                                    color=color.to_string()
                                    active=Signal::derive(move || filter_state.with(|s| s.statuses.contains(&status)))
                                    count=count
                                    on_click=Callback::new(move |_| toggle_status.run(status))
                                />
                            }.into_any()
                        }).collect_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn ActiveFilters(
    filter_state: Memo<FilterState>,
    page: Resource<Result<ChunkSetListPageDto, ServerFnError>>,
    toggle_status: Callback<ChunkSetStatusFilterDto>,
    on_clear: Callback<()>,
) -> impl IntoView {
    let has_filters = Memo::new(move |_| !filter_state.get().is_empty());

    view! {
        <Show when=move || has_filters.get() fallback=|| ()>
            <Transition fallback=|| ()>
                {move || {
                    let state = filter_state.get();
                    let total_match = page
                        .get()
                        .and_then(Result::ok)
                        .map_or(0, |p| p.total_matching);

                    view! {
                        <div class="list-page-active-filters">
                            <span>
                                {format!(
                                    "{total_match} match{}",
                                    if total_match == 1 { "" } else { "es" },
                                )}
                            </span>
                            {state.statuses.iter().copied().map(|status| {
                                let color = status_color(status);
                                view! {
                                    <FilterChip
                                        label=status.label().to_string()
                                        color=color.to_string()
                                        active=Signal::derive(move || true)
                                        show_remove=true
                                        on_click=Callback::new(move |_| toggle_status.run(status))
                                    />
                                }
                            }).collect_view()}
                            <button
                                type="button"
                                class="list-page-active-filters-clear"
                                on:click=move |_| on_clear.run(())
                            >
                                "Clear all"
                            </button>
                        </div>
                    }
                }}
            </Transition>
        </Show>
    }
}

fn status_color(status: ChunkSetStatusFilterDto) -> &'static str {
    match status {
        ChunkSetStatusFilterDto::Pinned => "var(--color-accent)",
        ChunkSetStatusFilterDto::Indexed | ChunkSetStatusFilterDto::UsedByEval => {
            "var(--status-ok)"
        }
        ChunkSetStatusFilterDto::Unused => "var(--color-text-muted)",
    }
}

#[component]
fn ChunkSetsTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[28%]">"Chunk set"</th>
                <th class="w-[18%]">"Document"</th>
                <th class="w-[10%] text-right">"Chunks"</th>
                <th class="w-[18%]">"Status"</th>
                <th class="w-[12%] text-right">"When"</th>
                <th class="w-[14%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn ChunkSetsTable(
    items: Vec<ChunkSetSummaryDto>,
    busy: ReadSignal<bool>,
    on_pin: Callback<(uuid::Uuid, bool)>,
    on_delete: Callback<uuid::Uuid>,
) -> impl IntoView {
    view! {
        <table class="data-table">
            <ChunkSetsTableHead />
            <tbody>
                {items.into_iter().map(|cs| view! {
                    <ChunkSetRow cs=cs busy=busy on_pin=on_pin on_delete=on_delete />
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn SkeletonChunkSetsTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <ChunkSetsTableHead />
            <tbody>
                <SkeletonRows columns=vec![
                    SkeletonColumn::with_sub("60%", "40%"),
                    SkeletonColumn::new("70%"),
                    SkeletonColumn::right("3rem"),
                    SkeletonColumn::new("60%"),
                    SkeletonColumn::right("4rem"),
                    SkeletonColumn::empty(),
                ] />
            </tbody>
        </table>
    }
}

#[component]
fn ChunkSetRow(
    cs: ChunkSetSummaryDto,
    busy: ReadSignal<bool>,
    on_pin: Callback<(uuid::Uuid, bool)>,
    on_delete: Callback<uuid::Uuid>,
) -> impl IntoView {
    let id = cs.chunk_set_id;
    let pinned = cs.pinned;
    let in_use = cs.in_use();
    let strategy = cs.chunking_config.strategy().as_str().to_string();
    let short_id = id.to_string().chars().take(8).collect::<String>();
    let when = format_when(&cs.created_at);
    let when_full = cs.created_at.clone();
    let chunk_count = cs.chunk_count;
    let indexing_refs = cs.indexing_refs;
    let variant_result_refs = cs.variant_result_refs;
    let doc_short = cs
        .document_id
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();

    let delete_title = if in_use {
        "In use — pinned or referenced; cannot delete"
    } else {
        "Delete"
    };

    let on_actions_click = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
    };

    view! {
        <tr>
            <td>
                <TitleCell
                    title=format!("cs-{short_id}")
                    sub=strategy
                />
            </td>
            <td>
                <span class="text-sm muted whitespace-nowrap">
                    {format!("doc-{doc_short} v{}", cs.document_version)}
                </span>
            </td>
            <td class="text-right font-mono text-sm">{chunk_count.to_string()}</td>
            <td>
                <div class="flex items-center gap-1 flex-wrap">
                    {pinned.then(|| view! {
                        <span class="pill pill-ok text-xs">"Pinned"</span>
                    })}
                    {(indexing_refs > 0).then(|| view! {
                        <span class="pill pill-ok text-xs">
                            {format!("indexed × {indexing_refs}")}
                        </span>
                    })}
                    {(variant_result_refs > 0).then(|| view! {
                        <span class="pill pill-ok text-xs">
                            {format!("eval × {variant_result_refs}")}
                        </span>
                    })}
                    {(!pinned && indexing_refs == 0 && variant_result_refs == 0).then(|| view! {
                        <span class="text-xs faint">"Unused"</span>
                    })}
                </div>
            </td>
            <td class="text-right text-xs muted whitespace-nowrap" title=when_full>{when}</td>
            <td class="text-right">
                <div class="inline-flex items-center gap-1" on:click=on_actions_click>
                    <button
                        type="button"
                        class="btn btn-ghost btn-compact"
                        prop:disabled=move || busy.get()
                        on:click=move |_| on_pin.run((id, !pinned))
                    >
                        {if pinned { "Unpin" } else { "Pin" }}
                    </button>
                    <button
                        type="button"
                        class="btn btn-ghost btn-compact"
                        title=delete_title
                        prop:disabled=move || busy.get() || in_use
                        on:click=move |_| on_delete.run(id)
                    >
                        "Delete"
                    </button>
                </div>
            </td>
        </tr>
    }
}
