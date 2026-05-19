pub mod redirect_by_id;

pub use redirect_by_id::DocumentByIdRedirect;

use std::collections::HashMap;

use leptos::either::Either;
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::query_signal;

use crate::server_functions::source_document::list_documents_page;
use crate::shared::contracts::{
    aggregate_type, DocumentListItemDto, DocumentListPageDto, DocumentListQueryDto,
    DocumentStatusFilterDto, SourceDescriptorDto, SourceFacetDto, SourceFilterDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::app::import_dialog::ImportDialog;
use crate::ui::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FilterState {
    search: String,
    sources: Vec<String>,
    statuses: Vec<DocumentStatusFilterDto>,
}

impl FilterState {
    fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.sources.is_empty() && self.statuses.is_empty()
    }

    fn to_query(&self, cursor: Option<String>) -> DocumentListQueryDto {
        DocumentListQueryDto {
            cursor,
            limit: PAGE_SIZE,
            search: Some(self.search.trim().to_string()).filter(|s| !s.is_empty()),
            sources: self
                .sources
                .iter()
                .filter_map(|key| SourceFilterDto::from_filter_key(key))
                .collect(),
            statuses: self.statuses.clone(),
        }
    }
}

#[component]
pub fn DocumentsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });

    let (search_query, set_search_query) = query_signal::<String>("q");
    let (sources_query, set_sources_query) = query_signal::<String>("source");
    let (statuses_query, set_statuses_query) = query_signal::<String>("status");

    let filter_state = Memo::new(move |_| FilterState {
        search: search_query.get().unwrap_or_default(),
        sources: sources_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        statuses: statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(DocumentStatusFilterDto::from_slug)
                    .collect()
            })
            .unwrap_or_default(),
    });

    let (cursor_stack, set_cursor_stack) = signal::<Vec<Option<String>>>(vec![None]);

    Effect::new(move |prev: Option<FilterState>| {
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
        |(state, cursor, _)| async move { list_documents_page(state.to_query(cursor)).await },
    );

    let (dialog_open, set_dialog_open) = signal(false);
    let open_signal: Signal<bool> = dialog_open.into();
    let close_dialog = Callback::new(move |_| set_dialog_open.set(false));

    let toggle_source = Callback::<String>::new(move |key: String| {
        let mut current = filter_state.get_untracked().sources;
        if let Some(pos) = current.iter().position(|s| s == &key) {
            current.remove(pos);
        } else {
            current.push(key);
        }
        let next = if current.is_empty() {
            None
        } else {
            Some(current.join(","))
        };
        set_sources_query.set(next);
    });

    let toggle_status = Callback::<DocumentStatusFilterDto>::new(move |status| {
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
        set_search_query.set(None);
        set_sources_query.set(None);
        set_statuses_query.set(None);
    };

    let on_search_input = move |ev: ev::Event| {
        let value = event_target_value(&ev);
        let next = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        set_search_query.set(next);
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
                        if let Err(err) = list_documents_page(state.to_query(Some(cursor))).await {
                            leptos::logging::warn!("documents prefetch failed: {err}");
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
                title="Documents"
                subtitle="Documents indexed into your RAG pipelines. Search, filter, and inspect their ingest status.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_dialog_open.set(true)
                    >
                        "+ Import"
                    </button>
                }.into_any())
            />

            <Surface flush=true>
                <DocumentsToolbar
                    search=Signal::derive(move || filter_state.get().search)
                    on_search_input=Callback::new(on_search_input)
                    on_clear_search=Callback::new(move |_| set_search_query.set(None))
                />
                <FacetBar
                    page=page
                    filter_state=filter_state
                    toggle_source=toggle_source
                    toggle_status=toggle_status
                />
                <ActiveFilters
                    filter_state=filter_state
                    page=page
                    toggle_source=toggle_source
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
                                            title="No documents yet"
                                            body="Import a document from a URL, an upload, or a configured connector.".to_string()
                                            action=Box::new(move || view! {
                                                <button
                                                    type="button"
                                                    class="btn btn-primary"
                                                    on:click=move |_| set_dialog_open.set(true)
                                                >
                                                    "Import document"
                                                </button>
                                            }.into_any())
                                        />
                                    </div>
                                }.into_any()
                            } else if page_data.items.is_empty() {
                                view! { <EmptyFilterState on_clear=Callback::new(clear_filters) /> }.into_any()
                            } else {
                                view! { <DocumentsTable docs=page_data.items.clone() /> }.into_any()
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

            <ImportDialog open=open_signal on_close=close_dialog />
        </div>
    }
}

#[component]
fn DocumentsToolbar(
    search: Signal<String>,
    on_search_input: Callback<ev::Event>,
    on_clear_search: Callback<()>,
) -> impl IntoView {
    let has_search = move || !search.get().trim().is_empty();
    view! {
        <div class="list-page-toolbar">
            <div class="list-page-search">
                <span class="list-page-search-icon" aria-hidden="true">"⌕"</span>
                <input
                    type="search"
                    placeholder="Search by title or URL…"
                    prop:value=move || search.get()
                    on:input=move |ev| on_search_input.run(ev)
                    autocomplete="off"
                    spellcheck="false"
                />
                {move || has_search().then(|| view! {
                    <button
                        type="button"
                        class="list-page-search-clear"
                        aria-label="Clear search"
                        on:click=move |_| on_clear_search.run(())
                    >"×"</button>
                })}
            </div>
        </div>
    }
}

#[component]
fn FacetBar(
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
    filter_state: Memo<FilterState>,
    toggle_source: Callback<String>,
    toggle_status: Callback<DocumentStatusFilterDto>,
) -> impl IntoView {
    let statuses: Signal<Vec<DocumentStatusFilterDto>> = Signal::derive(|| {
        vec![
            DocumentStatusFilterDto::Indexed,
            DocumentStatusFilterDto::InProgress,
            DocumentStatusFilterDto::Failed,
            DocumentStatusFilterDto::Registered,
        ]
    });

    view! {
        <div class="list-page-toolbar" style="border-top: none;">
            <div class="list-page-filter-group">
                <span class="list-page-filter-label">"Status"</span>
                {move || statuses.get().into_iter().map(|status| {
                    let is_active = filter_state.with(|s| s.statuses.contains(&status));
                    let color = status_color(status);
                    view! {
                        <button
                            type="button"
                            class="list-page-chip"
                            data-active=move || is_active.to_string()
                            on:click=move |_| toggle_status.run(status)
                        >
                            <span class="list-page-chip-dot" style=format!("background-color: {}", color)></span>
                            {status.label()}
                        </button>
                    }
                }).collect_view()}
            </div>

            <div class="list-page-filter-group">
                <span class="list-page-filter-label">"Source"</span>
                <Transition fallback=|| view! { <span class="faint text-xs">"…"</span> }>
                    {move || page.get().and_then(Result::ok).map(|p| {
                        let facets = p.sources.clone();
                        view! { <SourceFacets facets=facets filter_state=filter_state toggle_source=toggle_source /> }
                    })}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn SourceFacets(
    facets: Vec<SourceFacetDto>,
    filter_state: Memo<FilterState>,
    toggle_source: Callback<String>,
) -> impl IntoView {
    if facets.is_empty() {
        return view! { <span class="faint text-xs">"No sources yet"</span> }.into_any();
    }
    view! {
        <>
            {facets.into_iter().map(|facet| {
                let key = facet.source.filter_key();
                let label = facet.source.label();
                let count = facet.document_count;
                let key_check = key.clone();
                let is_active = filter_state.with(move |s| s.sources.contains(&key_check));
                let key_click = key.clone();
                view! {
                    <button
                        type="button"
                        class="list-page-chip"
                        data-active=move || is_active.to_string()
                        on:click=move |_| toggle_source.run(key_click.clone())
                    >
                        <SourceMark source=facet.source.clone() compact=true />
                        <span>{label}</span>
                        <span class="list-page-chip-count">{count.to_string()}</span>
                    </button>
                }
            }).collect_view()}
        </>
    }
    .into_any()
}

#[component]
fn ActiveFilters(
    filter_state: Memo<FilterState>,
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
    toggle_source: Callback<String>,
    toggle_status: Callback<DocumentStatusFilterDto>,
    on_clear: Callback<()>,
) -> impl IntoView {
    let has_filters = Memo::new(move |_| !filter_state.get().is_empty());

    view! {
        <Show when=move || has_filters.get() fallback=|| ()>
            <Transition fallback=|| ()>
                {move || {
                    let state = filter_state.get();
                    let (total_match, source_labels): (u64, HashMap<String, String>) = page
                        .get()
                        .and_then(Result::ok)
                        .map(|p| {
                            let labels = p
                                .sources
                                .iter()
                                .map(|f| (f.source.filter_key(), f.source.label()))
                                .collect();
                            (p.total_matching, labels)
                        })
                        .unwrap_or_default();

                    view! {
                        <div class="list-page-active-filters">
                            <span>
                                {format!(
                                    "{total_match} match{}",
                                    if total_match == 1 { "" } else { "es" },
                                )}
                            </span>
                            {(!state.search.is_empty())
                                .then(|| {
                                    view! {
                                        <span class="list-page-chip" data-active="true">
                                            <span class="faint">"Search:"</span>
                                            <span>{state.search.clone()}</span>
                                        </span>
                                    }
                                })}
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
                            {state
                                .sources
                                .iter()
                                .map(|key| {
                                    let label = source_labels
                                        .get(key)
                                        .cloned()
                                        .unwrap_or_else(|| key.clone());
                                    let key_click = key.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="list-page-chip"
                                            data-active="true"
                                            on:click=move |_| toggle_source.run(key_click.clone())
                                        >
                                            <span>{label}</span>
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
fn DocumentsTable(docs: Vec<DocumentListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-[42%]">"Title"</th>
                    <th>"Source"</th>
                    <th>"Status"</th>
                    <th class="text-right">"Version"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                {docs.into_iter().map(|doc| view! { <DocumentRow doc /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn DocumentRow(doc: DocumentListItemDto) -> impl IntoView {
    let href = format!(
        "/documents/{}/{}",
        doc.document_type.to_lowercase(),
        urlencoding::encode(&doc.source_ref_key),
    );
    let (status_label, status_kind) = ingest_status(&doc);
    let version_label = doc
        .latest_version
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "—".into());
    let title = doc.title.clone();
    let sub_text = match &doc.source {
        SourceDescriptorDto::Url { url, .. } => url.clone(),
        SourceDescriptorDto::Upload => format!("./{}", doc.source_ref_key),
    };
    let source = doc.source.clone();

    view! {
        <tr>
            <td>
                <A href=href.clone() attr:class="block">
                    <div class="list-page-title-cell">
                        <div class="list-page-title-text">{title}</div>
                        <div class="list-page-title-sub" title=sub_text.clone()>{sub_text.clone()}</div>
                    </div>
                </A>
            </td>
            <td><SourceMark source=source compact=false /></td>
            <td><StatusPill label=status_label kind=status_kind /></td>
            <td class="text-right text-xs muted">{version_label}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

#[component]
fn SourceMark(source: SourceDescriptorDto, compact: bool) -> impl IntoView {
    let (class_suffix, glyph, label) = match &source {
        SourceDescriptorDto::Upload => ("upload", "↥", "Upload".to_string()),
        SourceDescriptorDto::Url { host, .. } => ("url", "@", host.clone()),
    };
    let badge_class = format!("docs-source-badge docs-source-badge-{class_suffix}");
    let title_attr = match &source {
        SourceDescriptorDto::Upload => "Manual upload".to_string(),
        SourceDescriptorDto::Url { url, .. } => url.clone(),
    };

    if compact {
        view! {
            <span class="docs-source-badge-icon" title=title_attr.clone()>{glyph}</span>
        }
        .into_any()
    } else {
        view! {
            <span class=badge_class title=title_attr>
                <span class="docs-source-badge-icon">{glyph}</span>
                <span class="docs-source-badge-label">{label}</span>
            </span>
        }
        .into_any()
    }
}

#[component]
fn PaginationBar(
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
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
                    <th class="w-[42%]">"Title"</th>
                    <th>"Source"</th>
                    <th>"Status"</th>
                    <th class="text-right">"Version"</th>
                    <th class="w-8 text-right"></th>
                </tr>
            </thead>
            <tbody>
                {(0..6).map(|_| view! {
                    <tr class="list-page-skeleton-row">
                        <td>
                            <div class="list-page-skeleton-bar" style="width: 60%"></div>
                            <div class="list-page-skeleton-bar" style="width: 80%; margin-top: 0.3rem; height: 0.5rem"></div>
                        </td>
                        <td><div class="list-page-skeleton-bar" style="width: 6rem"></div></td>
                        <td><div class="list-page-skeleton-bar" style="width: 4rem"></div></td>
                        <td class="text-right"><div class="list-page-skeleton-bar" style="width: 2rem; display:inline-block"></div></td>
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
            <strong>"No documents match these filters"</strong>
            <p class="muted text-sm">"Try removing a filter or clearing your search."</p>
            <button
                type="button"
                class="btn"
                on:click=move |_| on_clear.run(())
            >"Clear filters"</button>
        </div>
    }
}

fn ingest_status(doc: &DocumentListItemDto) -> (String, Status) {
    if doc.document_id.is_none() {
        ("Not ingested".to_string(), Status::Stale)
    } else if doc.indexings.is_empty() {
        ("Registered".to_string(), Status::Info)
    } else if doc.indexings.iter().any(|i| i.status.contains("Indexed")) {
        ("Indexed".to_string(), Status::Ok)
    } else if doc.indexings.iter().any(|i| i.status.contains("Failed")) {
        ("Failed".to_string(), Status::Fail)
    } else if doc.indexings.iter().any(|i| i.status.contains("Embedding")) {
        ("Embedding".to_string(), Status::Pending)
    } else if doc.indexings.iter().any(|i| i.status.contains("Chunking")) {
        ("Chunking".to_string(), Status::Pending)
    } else {
        ("Pending".to_string(), Status::Pending)
    }
}

fn status_color(status: DocumentStatusFilterDto) -> &'static str {
    match status {
        DocumentStatusFilterDto::Indexed => "var(--status-ok)",
        DocumentStatusFilterDto::InProgress => "var(--status-pending)",
        DocumentStatusFilterDto::Failed => "var(--status-fail)",
        DocumentStatusFilterDto::Registered => "var(--status-info)",
    }
}
