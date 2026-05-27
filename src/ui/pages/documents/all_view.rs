use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;

use crate::server_functions::source_document::list_documents_page;
use crate::shared::contracts::{
    aggregate_type, DocumentListItemDto, DocumentListQueryDto, DocumentStatusFilterDto,
    SourceDescriptorDto, SourceFilterDto,
};
use crate::ui::components::primitives::{
    use_cursor_pagination, EmptyFilterState, EmptyState, PaginationBar, PaginationSummary,
    SkeletonColumn, SkeletonRows, Status, StatusPill, Surface, TitleCell,
};
use crate::ui::pages::documents::source_mark::SourceMark;
use crate::ui::pages::shared::indexing_summary;
use crate::ui::state::event_bus::use_invalidator;

use super::smart_filter::{ChipsState, SmartFilterBar};

const PAGE_SIZE: u32 = 25;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FilterState {
    search: String,
    sources: Vec<String>,
    statuses: Vec<DocumentStatusFilterDto>,
    connectors: Vec<uuid::Uuid>,
}

impl FilterState {
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
            connectors: self.connectors.clone(),
        }
    }

    fn chips(&self) -> ChipsState {
        ChipsState {
            statuses: self.statuses.clone(),
            sources: self.sources.clone(),
            connectors: self.connectors.clone(),
        }
    }
}

fn document_skeleton_columns() -> Vec<SkeletonColumn> {
    vec![
        SkeletonColumn::with_sub("60%", "80%"),
        SkeletonColumn::new("6rem"),
        SkeletonColumn::new("5rem"),
        SkeletonColumn::new("4rem"),
        SkeletonColumn::right("2rem"),
        SkeletonColumn::empty(),
    ]
}

#[component]
pub fn AllDocumentsView(open_import: Callback<()>) -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });

    let (search_query, set_search_query) = query_signal::<String>("q");
    let (sources_query, set_sources_query) = query_signal::<String>("source");
    let (statuses_query, set_statuses_query) = query_signal::<String>("status");
    let (connectors_query, set_connectors_query) = query_signal::<String>("connector");

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
        connectors: connectors_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(|p| uuid::Uuid::parse_str(p).ok())
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
        |(state, cursor, _)| async move { list_documents_page(state.to_query(cursor)).await },
    );

    let toggle_source = Callback::<String>::new(move |key: String| {
        let mut current = filter_state.get_untracked().sources;
        if let Some(pos) = current.iter().position(|s| s == &key) {
            current.remove(pos);
        } else {
            current.push(key);
        }
        set_sources_query.set((!current.is_empty()).then(|| current.join(",")));
    });

    let toggle_status = Callback::<DocumentStatusFilterDto>::new(move |status| {
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

    let toggle_connector = Callback::<uuid::Uuid>::new(move |connector_id| {
        let mut current = filter_state.get_untracked().connectors;
        if let Some(pos) = current.iter().position(|c| c == &connector_id) {
            current.remove(pos);
        } else {
            current.push(connector_id);
        }
        set_connectors_query.set((!current.is_empty()).then(|| {
            current
                .iter()
                .map(uuid::Uuid::to_string)
                .collect::<Vec<_>>()
                .join(",")
        }));
    });

    let on_search_change = Callback::<Option<String>>::new(move |value| {
        set_search_query.set(value);
    });

    let clear_filters = Callback::<()>::new(move |_| {
        set_search_query.set(None);
        set_sources_query.set(None);
        set_statuses_query.set(None);
        set_connectors_query.set(None);
    });

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

    let search_signal: Signal<String> = Signal::derive(move || filter_state.get().search);
    let chips_signal: Signal<ChipsState> = Signal::derive(move || filter_state.get().chips());
    let total_matching: Signal<Option<u64>> =
        Signal::derive(move || page.get().and_then(Result::ok).map(|p| p.total_matching));
    let summary: Signal<Option<PaginationSummary>> = Signal::derive(move || {
        page.get().and_then(Result::ok).map(|p| PaginationSummary {
            shown: p.items.len(),
            total_matching: p.total_matching,
            total_all: p.total_all,
            has_next: p.next_cursor.is_some(),
        })
    });

    view! {
        <Surface flush=true>
            <SmartFilterBar
                search=search_signal
                chips=chips_signal
                page=page
                total_matching=total_matching
                on_search_change=on_search_change
                on_toggle_status=toggle_status
                on_toggle_source=toggle_source
                on_toggle_connector=toggle_connector
                on_clear=clear_filters
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
                                        body="Import a document from a URL or an upload, or sync a configured connector.".to_string()
                                        action=Box::new(move || view! {
                                            <button
                                                type="button"
                                                class="btn btn-primary"
                                                on:click=move |_| open_import.run(())
                                            >
                                                "Import document"
                                            </button>
                                        }.into_any())
                                    />
                                </div>
                            }.into_any()
                        } else if page_data.items.is_empty() {
                            view! {
                                <EmptyFilterState
                                    title="No documents match these filters".to_string()
                                    body="Try removing a filter or clearing your search.".to_string()
                                    on_clear=clear_filters
                                />
                            }.into_any()
                        } else {
                            view! { <DocumentsTable docs=page_data.items.clone() /> }.into_any()
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
    }
}

#[component]
fn DocumentsTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[42%]">"Title"</th>
                <th class="w-[14%]">"Source"</th>
                <th class="w-[18%]">"Connector"</th>
                <th class="w-[14%]">"Status"</th>
                <th class="w-[8%] text-right">"Version"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn DocumentsTable(docs: Vec<DocumentListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <DocumentsTableHead />
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
    let connectors = doc.connectors.clone();

    let nav_href = href.clone();
    let on_row_click = move |ev: leptos::ev::MouseEvent| {
        if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
            return;
        }
        use_navigate()(&nav_href, NavigateOptions::default());
    };

    view! {
        <tr on:click=on_row_click>
            <td>
                <A href=href.clone() attr:class="block">
                    <TitleCell title=title sub=sub_text />
                </A>
            </td>
            <td><SourceMark source=source /></td>
            <td>
                {if connectors.is_empty() {
                    view! { <span class="faint text-xs">"—"</span> }.into_any()
                } else {
                    view! {
                        <div class="flex flex-wrap gap-1">
                            {connectors.into_iter().map(|c| view! {
                                <span class="pill pill-neutral text-xs" title=format!("Imported via {}", c.name)>
                                    {c.name.clone()}
                                </span>
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </td>
            <td><StatusPill label=status_label kind=status_kind /></td>
            <td class="text-right text-xs muted">{version_label}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

#[component]
fn SkeletonTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <DocumentsTableHead />
            <tbody>
                <SkeletonRows columns=document_skeleton_columns() />
            </tbody>
        </table>
    }
}

fn ingest_status(doc: &DocumentListItemDto) -> (String, Status) {
    if doc.document_id.is_none() {
        return ("Not ingested".to_string(), Status::Stale);
    }
    let summary = indexing_summary(&doc.indexings, doc.latest_version);
    (summary.label, summary.status)
}
