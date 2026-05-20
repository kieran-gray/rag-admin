use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;

use crate::server_functions::evaluation::get_evaluation_runs_page;
use crate::server_functions::source_document::list_documents;
use crate::shared::contracts::{
    aggregate_type, BestVariantDto, DocumentListItemDto, RecentEvaluationRunDto, RunListPageDto,
    RunListQueryDto, RunStatusFilterDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    Dialog, EmptyState, PageHeader, Status, StatusPill, Surface,
};

mod dataset_detail;
mod optimize_progress;
mod replicate_compare;
mod run_detail;

pub use dataset_detail::DatasetDetailPage;
pub use optimize_progress::OptimizeProgressPage;
pub use replicate_compare::ReplicateComparePage;
pub use run_detail::RunDetailPage;

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RunFilterState {
    statuses: Vec<RunStatusFilterDto>,
}

impl RunFilterState {
    fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    fn to_query(&self, cursor: Option<String>) -> RunListQueryDto {
        RunListQueryDto {
            cursor,
            limit: PAGE_SIZE,
            statuses: self.statuses.clone(),
        }
    }
}

#[component]
pub fn EvaluationsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));

    let (statuses_query, set_statuses_query) = query_signal::<String>("status");

    let filter_state = Memo::new(move |_| RunFilterState {
        statuses: statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(RunStatusFilterDto::from_slug)
                    .collect()
            })
            .unwrap_or_default(),
    });

    let (cursor_stack, set_cursor_stack) = signal::<Vec<Option<String>>>(vec![None]);

    Effect::new(move |prev: Option<RunFilterState>| {
        let current = filter_state.get();
        if let Some(p) = prev.as_ref() {
            if p != &current {
                set_cursor_stack.set(vec![None]);
            }
        }
        current
    });

    let current_cursor = Memo::new(move |_| {
        let stack = cursor_stack.get();
        stack.last().cloned().unwrap_or(None)
    });

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
        let next = if current.is_empty() {
            None
        } else {
            Some(
                current
                    .iter()
                    .map(|s| s.slug())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        };
        set_statuses_query.set(next);
    });

    let clear_filters = move |_| {
        set_statuses_query.set(None);
    };

    let next_page = move |_| {
        page.with(|res| {
            if let Some(Ok(p)) = res.as_ref() {
                if let Some(next) = p.next_cursor.clone() {
                    set_cursor_stack.update(|stack| stack.push(Some(next)));
                }
            }
        });
    };
    let prev_page = move |_| {
        set_cursor_stack.update(|stack| {
            if stack.len() > 1 {
                stack.pop();
            }
        });
    };
    let can_go_prev = move || cursor_stack.with(|s| s.len() > 1);
    let page_number = move || cursor_stack.with(Vec::len);

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
                />
                <ActiveFilters
                    filter_state=filter_state
                    page=page
                    toggle_status=toggle_status
                    on_clear=Callback::new(clear_filters)
                />

                <Transition fallback=move || view! { <SkeletonTable /> }>
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
                                view! { <EmptyFilterState on_clear=Callback::new(clear_filters) /> }.into_any()
                            } else {
                                view! { <RunsTable runs=page_data.items.clone() /> }.into_any()
                            }
                        })
                    })}
                </Transition>

                <PaginationBar
                    page=page
                    page_number=Signal::derive(page_number)
                    can_go_prev=Signal::derive(can_go_prev)
                    on_prev=Callback::new(prev_page)
                    on_next=Callback::new(next_page)
                />
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
) -> impl IntoView {
    let all_statuses = [
        RunStatusFilterDto::Running,
        RunStatusFilterDto::Pending,
        RunStatusFilterDto::Completed,
        RunStatusFilterDto::Failed,
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
                                <button
                                    type="button"
                                    class="list-page-chip"
                                    data-active=move || is_active.to_string()
                                    on:click=move |_| toggle_status.run(status)
                                >
                                    <span class="list-page-chip-dot" style=format!("background-color: {}", color)></span>
                                    <span>{status.label()}</span>
                                    <span class="list-page-chip-count">{count.to_string()}</span>
                                </button>
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
    filter_state: Memo<RunFilterState>,
    page: Resource<Result<RunListPageDto, ServerFnError>>,
    toggle_status: Callback<RunStatusFilterDto>,
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
                            {state
                                .statuses
                                .iter()
                                .copied()
                                .map(|status| {
                                    let color = status_color(status);
                                    view! {
                                        <button
                                            type="button"
                                            class="list-page-chip"
                                            data-active="true"
                                            on:click=move |_| toggle_status.run(status)
                                        >
                                            <span
                                                class="list-page-chip-dot"
                                                style=format!("background-color: {}", color)
                                            ></span>
                                            <span>{status.label()}</span>
                                            <span class="faint">"×"</span>
                                        </button>
                                    }
                                })
                                .collect_view()}
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

#[component]
fn RunsTable(runs: Vec<RecentEvaluationRunDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-[34%]">"Document"</th>
                    <th>"Status"</th>
                    <th class="w-[34%]">"Top variant"</th>
                    <th class="text-right">"Score"</th>
                    <th class="text-right">"When"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                {runs.into_iter().map(|r| view! { <RunRow run=r /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn RunRow(run: RecentEvaluationRunDto) -> impl IntoView {
    let href = format!("/runs/{}", run.run_id);
    let title = run
        .document_title
        .clone()
        .unwrap_or_else(|| "Untitled document".to_string());
    let title_attr = title.clone();
    let variant_count = run.variant_count;
    let variants_scored = run.variants_scored;
    let when = format_when(&run.created_at);
    let when_full = run.created_at.clone();
    let subtitle = variant_subtitle(&run.status, variants_scored, variant_count);

    view! {
        <tr>
            <td>
                <A href=href.clone() attr:class="block" attr:title=title_attr>
                    <div class="list-page-title-cell">
                        <div class="list-page-title-text">{title}</div>
                        <div class="list-page-title-sub">{subtitle}</div>
                    </div>
                </A>
            </td>
            <td>
                <RunStatusCell
                    status=run.status.clone()
                    variants_scored=variants_scored
                    variant_count=variant_count
                    failure_reason=run.failure_reason.clone()
                />
            </td>
            <td>
                {match run.best.as_ref() {
                    Some(best) => view! { <BestVariantCell best=best.clone() /> }.into_any(),
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right">
                {match run.best.as_ref() {
                    Some(best) => {
                        let score = best.score * 100.0;
                        view! {
                            <span class="font-mono text-text" style="font-size: 0.95rem">
                                {format!("{score:.1}%")}
                            </span>
                        }.into_any()
                    }
                    None => view! { <span class="faint text-xs">"—"</span> }.into_any(),
                }}
            </td>
            <td class="text-right text-xs muted" title=when_full>{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

#[component]
fn RunStatusCell(
    status: String,
    variants_scored: u32,
    variant_count: u32,
    failure_reason: Option<String>,
) -> impl IntoView {
    let (label, kind) = run_status_label(&status);
    let suffix = match status.as_str() {
        "running" => Some(format!("{variants_scored}/{variant_count}")),
        "pending" => (variant_count > 0).then(|| format!("0/{variant_count}")),
        _ => None,
    };
    let combined = match suffix {
        Some(s) => format!("{label} {s}"),
        None => label,
    };
    let title_attr = failure_reason.unwrap_or_default();
    view! {
        <span title=title_attr>
            <StatusPill label=combined kind=kind />
        </span>
    }
}

#[component]
fn BestVariantCell(best: BestVariantDto) -> impl IntoView {
    let label = best.label.clone();
    let metrics = best.metrics.clone();
    let top_k = best.options.top_k;
    let min_score = best.options.min_score();

    view! {
        <div class="flex flex-col gap-0.5 min-w-0">
            <div class="flex items-center gap-2 min-w-0">
                <span class="font-mono text-xs truncate" title=label.clone()>{label.clone()}</span>
                <span class="faint text-xs whitespace-nowrap">
                    {format!("k={top_k} · min {min_score:.2}")}
                </span>
            </div>
            <div class="faint text-xs font-mono whitespace-nowrap">
                {format!(
                    "R {:.0} · P {:.0} · IoU {:.0}",
                    metrics.recall_mean * 100.0,
                    metrics.precision_mean * 100.0,
                    metrics.iou_mean * 100.0,
                )}
            </div>
        </div>
    }
}

#[component]
fn PaginationBar(
    page: Resource<Result<RunListPageDto, ServerFnError>>,
    page_number: Signal<usize>,
    can_go_prev: Signal<bool>,
    on_prev: Callback<()>,
    on_next: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="list-page-pagination">
            <Transition fallback=|| view! { <span class="faint">"…"</span> }>
                {move || {
                    page.get()
                        .and_then(Result::ok)
                        .map(|p| {
                            let shown = p.items.len();
                            let matching = p.total_matching;
                            let all = p.total_all;
                            let extra = if matching == all {
                                format!("{shown} shown · {all} total")
                            } else {
                                format!("{shown} shown · {matching} of {all} match filters")
                            };
                            let has_next = p.next_cursor.is_some();
                            view! {
                                <>
                                    <span>{extra}</span>
                                    <span class="list-page-pagination-spacer"></span>
                                    <span class="list-page-indicator">
                                        {move || format!("Page {}", page_number.get())}
                                    </span>
                                    <button
                                        type="button"
                                        class="btn"
                                        prop:disabled=move || !can_go_prev.get()
                                        on:click=move |_| on_prev.run(())
                                    >
                                        "‹ Prev"
                                    </button>
                                    <button
                                        type="button"
                                        class="btn"
                                        prop:disabled=!has_next
                                        on:click=move |_| on_next.run(())
                                    >
                                        "Next ›"
                                    </button>
                                </>
                            }
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn SkeletonTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-[34%]">"Document"</th>
                    <th>"Status"</th>
                    <th class="w-[34%]">"Top variant"</th>
                    <th class="text-right">"Score"</th>
                    <th class="text-right">"When"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                {(0..6).map(|_| view! {
                    <tr class="list-page-skeleton-row">
                        <td>
                            <div class="list-page-skeleton-bar" style="width: 70%"></div>
                            <div class="list-page-skeleton-bar" style="width: 40%; margin-top: 0.3rem; height: 0.5rem"></div>
                        </td>
                        <td><div class="list-page-skeleton-bar" style="width: 5rem"></div></td>
                        <td>
                            <div class="list-page-skeleton-bar" style="width: 80%"></div>
                            <div class="list-page-skeleton-bar" style="width: 60%; margin-top: 0.3rem; height: 0.5rem"></div>
                        </td>
                        <td class="text-right"><div class="list-page-skeleton-bar" style="width: 3rem; display:inline-block"></div></td>
                        <td class="text-right"><div class="list-page-skeleton-bar" style="width: 4rem; display:inline-block"></div></td>
                        <td></td>
                    </tr>
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn EmptyFilterState(on_clear: Callback<()>) -> impl IntoView {
    view! {
        <div class="list-page-empty-filter">
            <strong>"No runs match these filters"</strong>
            <p class="muted text-sm">"Try removing a filter."</p>
            <button
                type="button"
                class="btn"
                on:click=move |_| on_clear.run(())
            >"Clear filters"</button>
        </div>
    }
}

#[component]
fn NewEvaluationDialog(#[prop(into)] open: Signal<bool>, on_close: Callback<()>) -> impl IntoView {
    let docs_invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let docs = Resource::new(
        move || (open.get(), docs_invalidator.get()),
        |(is_open, _)| async move {
            if !is_open {
                return Ok(Vec::new());
            }
            list_documents().await
        },
    );

    let navigate = use_navigate();
    let go_to_doc = Callback::new(move |doc: DocumentListItemDto| {
        let href = format!(
            "/evaluate/{}/{}",
            doc.document_type.to_lowercase(),
            urlencoding::encode(&doc.source_ref_key),
        );
        on_close.run(());
        navigate(&href, NavigateOptions::default());
    });

    view! {
        <Dialog
            open=open
            title="Run a new evaluation"
            subtitle="Pick a document. The evaluation workflow opens at the dataset step.".to_string()
            on_close=on_close
        >
            <Suspense fallback=|| view! { <p class="muted text-sm">"Loading documents…"</p> }>
                {move || docs.get().map(|res| match res {
                    Err(e) => view! {
                        <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                    }.into_any(),
                    Ok(list) => {
                        let imported: Vec<DocumentListItemDto> = list
                            .into_iter()
                            .filter(|d| d.document_id.is_some())
                            .collect();
                        if imported.is_empty() {
                            view! {
                                <p class="muted text-sm">
                                    "No imported documents yet. Import one from the Documents page first."
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <ul class="flex flex-col gap-1">
                                    {imported.into_iter().map(|d| view! {
                                        <DocumentPickerRow doc=d on_pick=go_to_doc />
                                    }).collect_view()}
                                </ul>
                            }.into_any()
                        }
                    }
                })}
            </Suspense>
        </Dialog>
    }
}

#[component]
fn DocumentPickerRow(
    doc: DocumentListItemDto,
    on_pick: Callback<DocumentListItemDto>,
) -> impl IntoView {
    let title = doc.title.clone();
    let source_ref = doc.source_ref_key.clone();
    let type_label = document_type_label(&doc.document_type).to_string();
    let doc_for_click = doc.clone();

    view! {
        <li>
            <button
                type="button"
                class="w-full flex items-center justify-between gap-3 px-3 py-2 rounded text-left hover:bg-[var(--color-surface-hover)]"
                on:click=move |_| on_pick.run(doc_for_click.clone())
            >
                <div class="min-w-0">
                    <div class="text-sm truncate">{title}</div>
                    <div class="faint text-xs truncate">{format!("./{source_ref}")}</div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    <span class="pill pill-neutral text-xs">{type_label}</span>
                    <span class="faint">"›"</span>
                </div>
            </button>
        </li>
    }
}

fn document_type_label(doc_type: &str) -> &'static str {
    match doc_type {
        "Markdown" => "Markdown",
        "PlainText" => "Plain text",
        "WebPage" => "Web page",
        "Pdf" => "PDF",
        _ => "Document",
    }
}

fn run_status_label(status: &str) -> (String, Status) {
    match status {
        "completed" => ("Completed".to_string(), Status::Ok),
        "failed" => ("Failed".to_string(), Status::Fail),
        "running" => ("Running".to_string(), Status::Pending),
        "pending" => ("Pending".to_string(), Status::Pending),
        _ => ("Unknown".to_string(), Status::Neutral),
    }
}

fn status_color(status: RunStatusFilterDto) -> &'static str {
    match status {
        RunStatusFilterDto::Completed => "var(--status-ok)",
        RunStatusFilterDto::Running | RunStatusFilterDto::Pending => "var(--status-pending)",
        RunStatusFilterDto::Failed => "var(--status-fail)",
    }
}

fn variant_subtitle(status: &str, scored: u32, total: u32) -> String {
    if status == "running" || status == "pending" {
        if total == 0 {
            "preparing variants".to_string()
        } else {
            format!("{scored} of {total} variants scored")
        }
    } else if total == 1 {
        "1 variant".to_string()
    } else {
        format!("{total} variants")
    }
}

fn format_when(ts: &str) -> String {
    let Some((date, time)) = ts.split_once('T') else {
        return ts.to_string();
    };
    let mut parts = date.split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return ts.to_string();
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return ts.to_string();
    };
    let hm = time.get(..5).unwrap_or("");
    let month_label = month_short(month);
    format!("{month_label} {day}, {year} · {hm}")
}

fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "?",
    }
}
