use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::query_signal;

use crate::server_functions::evaluation::get_evaluation_runs_page;
use crate::shared::contracts::{
    aggregate_type, RunKindDto, RunListPageDto, RunListQueryDto, RunStatusFilterDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    use_cursor_pagination, EmptyFilterState, EmptyState, FilterChip, PageHeader, PaginationBar,
    PaginationSummary, Surface,
};

mod dataset_detail;
mod new_run_dialog;
mod run_detail;
mod run_view;
mod runs_table;

pub use dataset_detail::DatasetDetailPage;
pub use new_run_dialog::NewEvaluationDialog;
pub use run_view::RunPage;
use runs_table::{status_color, RunsTable, SkeletonRunsTable};

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RunFilterState {
    statuses: Vec<RunStatusFilterDto>,
    kinds: Vec<RunKindDto>,
}

impl RunFilterState {
    fn is_empty(&self) -> bool {
        self.statuses.is_empty() && self.kinds.is_empty()
    }

    fn to_query(&self, cursor: Option<String>) -> RunListQueryDto {
        RunListQueryDto {
            cursor,
            limit: PAGE_SIZE,
            statuses: self.statuses.clone(),
            kinds: self.kinds.clone(),
        }
    }
}

#[component]
pub fn EvaluationsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));

    let (statuses_query, set_statuses_query) = query_signal::<String>("status");
    let (kinds_query, set_kinds_query) = query_signal::<String>("kind");

    let filter_state = Memo::new(move |_| RunFilterState {
        statuses: statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(RunStatusFilterDto::from_slug)
                    .collect()
            })
            .unwrap_or_default(),
        kinds: kinds_query
            .get()
            .map(|s| s.split(',').filter_map(RunKindDto::from_slug).collect())
            .unwrap_or_default(),
    });

    let pagination = use_cursor_pagination();
    let pagination_reset = pagination.reset;

    Effect::new(move |prev: Option<RunFilterState>| {
        let current = filter_state.get();
        if let Some(p) = prev.as_ref() {
            if p != &current {
                pagination_reset.run(());
            }
        }
        current
    });

    let current_cursor = pagination.current_cursor;
    let page = Resource::new(
        move || (filter_state.get(), current_cursor.get(), invalidator.get()),
        |(state, cursor, _)| async move { get_evaluation_runs_page(state.to_query(cursor)).await },
    );

    let (launcher_open, set_launcher_open) = signal(false);
    let launcher_open_signal: Signal<bool> = launcher_open.into();
    let close_launcher = Callback::new(move |_| set_launcher_open.set(false));
    let open_launcher = move |_| set_launcher_open.set(true);

    let toggle_status = Callback::<RunStatusFilterDto>::new(move |status| {
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

    let toggle_kind = Callback::<RunKindDto>::new(move |kind| {
        let mut current = filter_state.get_untracked().kinds;
        if let Some(pos) = current.iter().position(|k| k == &kind) {
            current.remove(pos);
        } else {
            current.push(kind);
        }
        set_kinds_query.set((!current.is_empty()).then(|| {
            current
                .iter()
                .map(|k| k.slug())
                .collect::<Vec<_>>()
                .join(",")
        }));
    });

    let clear_filters = move |_| {
        set_statuses_query.set(None);
        set_kinds_query.set(None);
    };

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

    let prefetch_next = move || {
        page.with(|res| {
            if let Some(Ok(p)) = res.as_ref() {
                if let Some(cursor) = p.next_cursor.clone() {
                    let state = filter_state.get_untracked();
                    spawn_local(async move {
                        if let Err(err) =
                            get_evaluation_runs_page(state.to_query(Some(cursor))).await
                        {
                            leptos::logging::warn!("runs prefetch failed: {err}");
                        }
                    });
                }
            }
        });
    };

    Effect::new(move |_| {
        prefetch_next();
    });

    let summary: Signal<Option<PaginationSummary>> = Signal::derive(move || {
        page.get().and_then(Result::ok).map(|p| PaginationSummary {
            shown: p.items.len(),
            total_matching: p.total_matching,
            total_all: p.total_all,
            has_next: p.next_cursor.is_some(),
        })
    });

    view! {
        <div>
            <PageHeader
                title="Evaluations"
                subtitle="Evaluation runs and their top-scoring chunking configuration."
                    .to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=open_launcher
                    >
                        "+ New evaluation"
                    </button>
                }.into_any())
            />

            <Surface flush=true>
                <FilterBar
                    page=page
                    filter_state=filter_state
                    toggle_status=toggle_status
                    toggle_kind=toggle_kind
                />
                <ActiveFilters
                    filter_state=filter_state
                    page=page
                    toggle_status=toggle_status
                    toggle_kind=toggle_kind
                    on_clear=Callback::new(clear_filters)
                />

                <Transition fallback=move || view! { <SkeletonRunsTable /> }>
                    {move || page.get().map(|res| match res {
                        Err(e) => Either::Left(view! {
                            <div class="p-6 log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        }),
                        Ok(page_data) => Either::Right({
                            if page_data.items.is_empty() && page_data.total_all == 0 {
                                view! {
                                    <div class="p-6">
                                        <EmptyState
                                            title="No evaluation runs yet"
                                            body="Pick a document and launch a run from the button above. Results land here as they complete.".to_string()
                                        />
                                    </div>
                                }.into_any()
                            } else if page_data.items.is_empty() {
                                view! {
                                    <EmptyFilterState
                                        title="No runs match these filters".to_string()
                                        body="Try removing a filter.".to_string()
                                        on_clear=Callback::new(clear_filters)
                                    />
                                }.into_any()
                            } else {
                                view! { <RunsTable runs=page_data.items.clone() /> }.into_any()
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

            <NewEvaluationDialog open=launcher_open_signal on_close=close_launcher />
        </div>
    }
}

#[component]
fn FilterBar(
    page: Resource<Result<RunListPageDto, ServerFnError>>,
    filter_state: Memo<RunFilterState>,
    toggle_status: Callback<RunStatusFilterDto>,
    toggle_kind: Callback<RunKindDto>,
) -> impl IntoView {
    let all_statuses = [
        RunStatusFilterDto::Running,
        RunStatusFilterDto::Pending,
        RunStatusFilterDto::Completed,
        RunStatusFilterDto::Failed,
    ];
    let all_kinds = [RunKindDto::Optimization, RunKindDto::Manual];

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
            <div class="list-page-filter-group">
                <span class="list-page-filter-label">"Kind"</span>
                <Transition fallback=|| view! { <span class="faint text-xs">"…"</span> }>
                    {move || page.get().and_then(Result::ok).map(move |p| {
                        all_kinds.iter().map(|kind| {
                            let kind = *kind;
                            let count = p.kind_counts.iter()
                                .find(|f| f.kind == kind)
                                .map(|f| f.count)
                                .unwrap_or(0);
                            let is_active = filter_state.with(|s| s.kinds.contains(&kind));
                            let color = kind_color(kind);
                            if count == 0 && !is_active {
                                return ().into_any();
                            }
                            view! {
                                <FilterChip
                                    label=kind.label().to_string()
                                    color=color.to_string()
                                    active=Signal::derive(move || filter_state.with(|s| s.kinds.contains(&kind)))
                                    count=count
                                    on_click=Callback::new(move |_| toggle_kind.run(kind))
                                />
                            }.into_any()
                        }).collect_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

pub fn kind_color(kind: RunKindDto) -> &'static str {
    match kind {
        RunKindDto::Optimization => "var(--color-accent)",
        RunKindDto::Manual => "var(--color-text-muted)",
    }
}

#[component]
fn ActiveFilters(
    filter_state: Memo<RunFilterState>,
    page: Resource<Result<RunListPageDto, ServerFnError>>,
    toggle_status: Callback<RunStatusFilterDto>,
    toggle_kind: Callback<RunKindDto>,
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
                            {state.kinds.iter().copied().map(|kind| {
                                let color = kind_color(kind);
                                view! {
                                    <FilterChip
                                        label=kind.label().to_string()
                                        color=color.to_string()
                                        active=Signal::derive(move || true)
                                        show_remove=true
                                        on_click=Callback::new(move |_| toggle_kind.run(kind))
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
