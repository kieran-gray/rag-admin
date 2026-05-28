use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{query_signal, use_navigate};
use leptos_router::NavigateOptions;

use crate::server_functions::comprehension::list_all_document_maps;
use crate::shared::contracts::{aggregate_type, DocumentMapListItemDto};
use crate::ui::components::primitives::{
    EmptyFilterState, EmptyState, FilterChip, PageHeader, SkeletonColumn, SkeletonRows, Status,
    StatusPill, Surface, TitleCell,
};
use crate::ui::pages::document_detail::map_phase::{section_count, MapPhase, MapProgress};
use crate::ui::state::event_bus::use_invalidator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapStatusFilter {
    Ready,
    Building,
}

impl MapStatusFilter {
    const ALL: [Self; 2] = [Self::Ready, Self::Building];

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Building => "Building",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Building => "building",
        }
    }

    fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "ready" => Some(Self::Ready),
            "building" => Some(Self::Building),
            _ => None,
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Ready => "var(--status-ok)",
            Self::Building => "var(--status-pending)",
        }
    }

    fn matches(self, phase: MapPhase) -> bool {
        match self {
            Self::Ready => phase.is_ready(),
            Self::Building => !phase.is_ready(),
        }
    }
}

#[component]
pub fn MapsPage() -> impl IntoView {
    let invalidator =
        use_invalidator(|e| e.from_any(&["document_map", aggregate_type::SOURCE_DOCUMENT]));
    let (statuses_query, set_statuses_query) = query_signal::<String>("status");

    let active_statuses = Memo::new(move |_| -> Vec<MapStatusFilter> {
        statuses_query
            .get()
            .map(|s| {
                s.split(',')
                    .filter_map(MapStatusFilter::from_slug)
                    .collect()
            })
            .unwrap_or_default()
    });

    let maps = Resource::new(
        move || invalidator.get(),
        |_| async move { list_all_document_maps().await.map_err(|e| e.to_string()) },
    );

    let toggle_status = Callback::<MapStatusFilter>::new(move |status| {
        let mut current = active_statuses.get_untracked();
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

    let clear_filters = Callback::<()>::new(move |_| set_statuses_query.set(None));

    view! {
        <div>
            <PageHeader
                title="Maps"
                subtitle="Document summaries: observations, threads, and insights extracted per document version. Used to generate evaluation questions.".to_string()
                actions=Box::new(move || view! {
                    <A href="/workflows/evaluate" attr:class="btn btn-primary">
                        "+ New map"
                    </A>
                }.into_any())
            />

            <Surface flush=true>
                <Transition fallback=move || view! { <SkeletonMapsTable /> }>
                    {move || maps.get().map(|res| match res {
                        Err(e) => Either::Left(view! {
                            <div class="p-6 log-line-error text-sm">
                                {format!("Failed to load: {e}")}
                            </div>
                        }),
                        Ok(items) => Either::Right({
                            let total_all = items.len();
                            let counts = status_counts(&items);
                            let active = active_statuses.get();
                            let filtered: Vec<DocumentMapListItemDto> = if active.is_empty() {
                                items
                            } else {
                                items.into_iter().filter(|item| {
                                    let phase = MapPhase::from_status(&item.status);
                                    active.iter().any(|s| s.matches(phase))
                                }).collect()
                            };

                            view! {
                                <FilterBar
                                    counts=counts
                                    active_statuses=active_statuses
                                    toggle_status=toggle_status
                                />
                                <ActiveFilters
                                    active_statuses=active_statuses
                                    total_matching=filtered.len()
                                    toggle_status=toggle_status
                                    on_clear=clear_filters
                                />
                                {
                                    if total_all == 0 {
                                        view! {
                                            <div class="p-6">
                                                <EmptyState
                                                    title="No maps yet"
                                                    body="Maps are built per document version. Open a document and use the map card to build one.".to_string()
                                                />
                                            </div>
                                        }.into_any()
                                    } else if filtered.is_empty() {
                                        view! {
                                            <EmptyFilterState
                                                title="No maps match these filters".to_string()
                                                body="Try removing a filter.".to_string()
                                                on_clear=clear_filters
                                            />
                                        }.into_any()
                                    } else {
                                        view! { <MapsTable items=filtered /> }.into_any()
                                    }
                                }
                            }.into_any()
                        }),
                    })}
                </Transition>
            </Surface>

        </div>
    }
}

#[component]
fn FilterBar(
    counts: Vec<(MapStatusFilter, usize)>,
    active_statuses: Memo<Vec<MapStatusFilter>>,
    toggle_status: Callback<MapStatusFilter>,
) -> impl IntoView {
    view! {
        <div class="list-page-toolbar">
            <div class="list-page-filter-group">
                <span class="list-page-filter-label">"Status"</span>
                {MapStatusFilter::ALL.iter().map(|status| {
                    let status = *status;
                    let count = counts.iter().find(|(s, _)| *s == status).map(|(_, c)| *c).unwrap_or(0);
                    let is_active = active_statuses.with_untracked(|s| s.contains(&status));
                    if count == 0 && !is_active {
                        return ().into_any();
                    }
                    view! {
                        <FilterChip
                            label=status.label().to_string()
                            color=status.color().to_string()
                            active=Signal::derive(move || active_statuses.with(|s| s.contains(&status)))
                            count=count as u64
                            on_click=Callback::new(move |_| toggle_status.run(status))
                        />
                    }.into_any()
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn ActiveFilters(
    active_statuses: Memo<Vec<MapStatusFilter>>,
    total_matching: usize,
    toggle_status: Callback<MapStatusFilter>,
    on_clear: Callback<()>,
) -> impl IntoView {
    let has_filters = Memo::new(move |_| !active_statuses.get().is_empty());
    view! {
        <Show when=move || has_filters.get() fallback=|| ()>
            <div class="list-page-active-filters">
                <span>
                    {format!(
                        "{total_matching} match{}",
                        if total_matching == 1 { "" } else { "es" },
                    )}
                </span>
                {move || active_statuses.get().into_iter().map(|status| view! {
                    <FilterChip
                        label=status.label().to_string()
                        color=status.color().to_string()
                        active=Signal::derive(move || true)
                        show_remove=true
                        on_click=Callback::new(move |_| toggle_status.run(status))
                    />
                }).collect_view()}
                <button
                    type="button"
                    class="list-page-active-filters-clear"
                    on:click=move |_| on_clear.run(())
                >
                    "Clear all"
                </button>
            </div>
        </Show>
    }
}

fn status_counts(items: &[DocumentMapListItemDto]) -> Vec<(MapStatusFilter, usize)> {
    let mut ready = 0usize;
    let mut building = 0usize;
    for item in items {
        let phase = MapPhase::from_status(&item.status);
        if phase.is_ready() {
            ready += 1;
        } else {
            building += 1;
        }
    }
    vec![
        (MapStatusFilter::Ready, ready),
        (MapStatusFilter::Building, building),
    ]
}

#[component]
fn MapsTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[44%]">"Document"</th>
                <th class="w-[20%]">"Status"</th>
                <th class="w-[32%]">"Synthesis"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn MapsTable(items: Vec<DocumentMapListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <MapsTableHead />
            <tbody>
                {items.into_iter().map(|m| view! { <MapRow item=m /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn SkeletonMapsTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <MapsTableHead />
            <tbody>
                <SkeletonRows columns=vec![
                    SkeletonColumn::with_sub("60%", "40%"),
                    SkeletonColumn::new("70%"),
                    SkeletonColumn::new("80%"),
                    SkeletonColumn::empty(),
                ] />
            </tbody>
        </table>
    }
}

#[component]
fn MapRow(item: DocumentMapListItemDto) -> impl IntoView {
    let phase = MapPhase::from_status(&item.status);
    let (status_kind, status_label) = pill_for(phase, &item);

    let title = if item.document_title.is_empty() {
        format!("doc-{}", short(&item.document_id.to_string()))
    } else {
        item.document_title.clone()
    };
    let sub = if item.source_ref_key.is_empty() {
        format!("v{}", item.document_version)
    } else {
        format!("{} · v{}", item.source_ref_key, item.document_version)
    };

    let map_href = if item.document_type.is_empty() || item.source_ref_key.is_empty() {
        None
    } else {
        Some(format!(
            "/documents/{}/{}/map",
            item.document_type,
            urlencoding::encode(&item.source_ref_key),
        ))
    };

    let sections = section_count(item.chunk_count, item.section_size);
    let synthesis_line = if phase.is_ready() {
        format!(
            "{} insight{} · {} thread{} · {} obs",
            item.insights_synthesized,
            plural(item.insights_synthesized),
            item.threads_synthesized,
            plural(item.threads_synthesized),
            item.observations_extracted,
        )
    } else {
        format!(
            "obs {}/{} · threads {}/{}",
            item.observations_extracted, item.chunk_count, item.threads_synthesized, sections,
        )
    };

    let synthesis_class = "text-xs muted font-mono";

    let nav_href = map_href.clone();
    let on_row_click = move |ev: leptos::ev::MouseEvent| {
        if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
            return;
        }
        if let Some(href) = nav_href.as_ref() {
            use_navigate()(href, NavigateOptions::default());
        }
    };

    let row_class = if map_href.is_some() {
        ""
    } else {
        "is-disabled"
    };

    view! {
        <tr class=row_class on:click=on_row_click>
            <td>
                {match map_href.clone() {
                    Some(href) => Either::Left(view! {
                        <A href=href attr:class="block">
                            <TitleCell title=title sub=sub />
                        </A>
                    }),
                    None => Either::Right(view! { <TitleCell title=title sub=sub /> }),
                }}
            </td>
            <td>
                <StatusPill label=status_label kind=status_kind />
            </td>
            <td>
                <span class=synthesis_class>{synthesis_line}</span>
            </td>
            <td class="text-right faint">{map_href.map(|_| "›")}</td>
        </tr>
    }
}

fn pill_for(phase: MapPhase, item: &DocumentMapListItemDto) -> (Status, String) {
    phase.pill(MapProgress {
        observations_extracted: item.observations_extracted,
        chunk_count: item.chunk_count,
        threads_synthesized: item.threads_synthesized,
        section_size: item.section_size,
    })
}

fn plural(n: u32) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn short(s: &str) -> String {
    s.chars().take(8).collect()
}
