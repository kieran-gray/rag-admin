use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;

use crate::server_functions::evaluation::get_evaluation_datasets_page;
use crate::shared::contracts::{
    aggregate_type, DatasetListItemDto, DatasetListPageDto, DatasetListQueryDto,
    DatasetStatusFilterDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    use_cursor_pagination, EmptyFilterState, EmptyState, FilterChip, PageHeader, PaginationBar,
    PaginationSummary, SkeletonColumn, SkeletonRows, Status, StatusPill, Surface, TitleCell,
};
use crate::ui::pages::shared::format_when;

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FilterState {
    statuses: Vec<DatasetStatusFilterDto>,
}

impl FilterState {
    fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    fn to_query(&self, cursor: Option<String>) -> DatasetListQueryDto {
        DatasetListQueryDto {
            cursor,
            limit: PAGE_SIZE,
            statuses: self.statuses.clone(),
        }
    }
}

#[component]
pub fn DatasetsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_DATASET]));

    let (statuses_query, set_statuses_query) = query_signal::<String>("status");

    let filter_state = Memo::new(move |_| FilterState {
        statuses: statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(DatasetStatusFilterDto::from_slug)
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
    let page = Resource::new(
        move || (filter_state.get(), current_cursor.get(), invalidator.get()),
        |(state, cursor, _)| async move { get_evaluation_datasets_page(state.to_query(cursor)).await },
    );

    let toggle_status = Callback::<DatasetStatusFilterDto>::new(move |status| {
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

    let prefetch_next = move || {
        page.with(|res| {
            if let Some(Ok(p)) = res.as_ref() {
                if let Some(cursor) = p.next_cursor.clone() {
                    let state = filter_state.get_untracked();
                    spawn_local(async move {
                        if let Err(err) =
                            get_evaluation_datasets_page(state.to_query(Some(cursor))).await
                        {
                            leptos::logging::warn!("datasets prefetch failed: {err}");
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
                title="Datasets"
                eyebrow="Evaluations".to_string()
                subtitle="Question datasets generated for evaluation runs.".to_string()
            />

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

                <Transition fallback=move || view! { <SkeletonDatasetsTable /> }>
                    {move || page.get().map(|res| match res {
                        Err(e) => Either::Left(view! {
                            <div class="p-6 log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        }),
                        Ok(page_data) => Either::Right({
                            if page_data.items.is_empty() && page_data.total_all == 0 {
                                view! {
                                    <div class="p-6">
                                        <EmptyState
                                            title="No datasets yet"
                                            body="Datasets are created from the Evaluate workflow. Open a document, launch the workflow, and generate a question set.".to_string()
                                        />
                                    </div>
                                }.into_any()
                            } else if page_data.items.is_empty() {
                                view! {
                                    <EmptyFilterState
                                        title="No datasets match these filters".to_string()
                                        body="Try removing a filter.".to_string()
                                        on_clear=Callback::new(clear_filters)
                                    />
                                }.into_any()
                            } else {
                                view! { <DatasetsTable items=page_data.items.clone() /> }.into_any()
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
fn DatasetsTable(items: Vec<DatasetListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <DatasetsTableHead />
            <tbody>
                {items.into_iter().map(|d| view! { <DatasetRow item=d /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn SkeletonDatasetsTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <DatasetsTableHead />
            <tbody>
                <SkeletonRows columns=vec![
                    SkeletonColumn::with_sub("70%", "40%"),
                    SkeletonColumn::new("5rem"),
                    SkeletonColumn::right("3rem"),
                    SkeletonColumn::right("3rem"),
                    SkeletonColumn::right("4rem"),
                    SkeletonColumn::empty(),
                ] />
            </tbody>
        </table>
    }
}

#[component]
fn DatasetsTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[44%]">"Dataset"</th>
                <th class="w-[14%]">"Status"</th>
                <th class="w-[10%] text-right">"Questions"</th>
                <th class="w-[8%] text-right">"Runs"</th>
                <th class="w-[16%] text-right">"When"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn DatasetRow(item: DatasetListItemDto) -> impl IntoView {
    let dataset_id = item.dataset_id;
    let href = format!("/evaluations/datasets/{dataset_id}");
    let nav_href = href.clone();
    let on_row_click = move |ev: leptos::ev::MouseEvent| {
        if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
            return;
        }
        use_navigate()(&nav_href, NavigateOptions::default());
    };

    let label = if item.label.is_empty() {
        format!(
            "dataset-{}",
            item.dataset_id
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        )
    } else {
        item.label.clone()
    };
    let document_title = item
        .document_title
        .clone()
        .unwrap_or_else(|| format!("doc-{}", short_id(item.document_id)));
    let subtitle = format!("{document_title}  ·  {}", item.generation_model);

    let (status_label, status_kind) = status_pill(&item.status);
    let status_title = item.failure_reason.clone().unwrap_or_default();
    let when = format_when(&item.created_at);
    let when_full = item.created_at.clone();
    let questions_label =
        if item.target_question_count > 0 && item.question_count < item.target_question_count {
            format!("{}/{}", item.question_count, item.target_question_count)
        } else {
            item.question_count.to_string()
        };

    view! {
        <tr on:click=on_row_click>
            <td>
                <A href=href.clone() attr:class="block">
                    <TitleCell title=label sub=subtitle />
                </A>
            </td>
            <td>
                <span title=status_title>
                    <StatusPill label=status_label kind=status_kind />
                </span>
            </td>
            <td class="text-right font-mono text-sm">{questions_label}</td>
            <td class="text-right font-mono text-sm">{item.run_count.to_string()}</td>
            <td class="text-right text-xs muted whitespace-nowrap" title=when_full>{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

fn short_id(id: uuid::Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

fn status_pill(status: &str) -> (String, Status) {
    match status {
        "completed" => ("Completed".to_string(), Status::Ok),
        "generating" => ("Generating".to_string(), Status::Pending),
        "failed" => ("Failed".to_string(), Status::Fail),
        "cancelled" => ("Cancelled".to_string(), Status::Cancel),
        other => (other.to_string(), Status::Neutral),
    }
}

#[component]
fn FilterBar(
    page: Resource<Result<DatasetListPageDto, ServerFnError>>,
    filter_state: Memo<FilterState>,
    toggle_status: Callback<DatasetStatusFilterDto>,
) -> impl IntoView {
    let all_statuses = [
        DatasetStatusFilterDto::Generating,
        DatasetStatusFilterDto::Completed,
        DatasetStatusFilterDto::Failed,
        DatasetStatusFilterDto::Cancelled,
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
    page: Resource<Result<DatasetListPageDto, ServerFnError>>,
    toggle_status: Callback<DatasetStatusFilterDto>,
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

fn status_color(status: DatasetStatusFilterDto) -> &'static str {
    match status {
        DatasetStatusFilterDto::Completed => "var(--status-ok)",
        DatasetStatusFilterDto::Generating => "var(--status-pending)",
        DatasetStatusFilterDto::Failed => "var(--status-fail)",
        DatasetStatusFilterDto::Cancelled => "var(--color-text-muted)",
    }
}
