use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{QueryHit, QueryResult};
use crate::ui::components::playground::marks_store::MarkKind;
use crate::ui::components::playground::{HitCard, LatencyBadge, ScoreSparkline};
use crate::ui::components::primitives::{EmptyState, Surface};
use crate::ui::pages::shared::{hit_display_title, hit_source_link};

#[component]
pub(super) fn ResultsPanel(
    result: QueryResult,
    query_text: String,
    profile_id: Uuid,
    min_score: f32,
    hit_filter: RwSignal<String>,
    highlighted_hit: RwSignal<Option<usize>>,
    mark_kind_for: impl Fn(&str, Uuid) -> Option<MarkKind> + Copy + Send + Sync + 'static,
    cycle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
    on_pin_baseline: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let hits = result.hits;
    let timings = result.timings;
    if hits.is_empty() {
        return view! {
            <Surface title="Results".to_string()>
                <EmptyState
                    title="No matches"
                    body="No chunks scored above the minimum.".to_string()
                />
            </Surface>
        }
        .into_any();
    }
    let count = hits.len();
    let scores: Vec<f32> = hits.iter().map(|h| h.score).collect();

    view! {
        <Surface
            title=format!("Results · {count}")
            actions=Box::new(move || view! {
                <div class="results-header-actions">
                    <ScoreSparkline scores=scores.clone() min_score=min_score />
                    <LatencyBadge timings=timings />
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        title="Pin this run as baseline for comparison"
                        on:click=move |_| on_pin_baseline()
                    >
                        "Pin as baseline"
                    </button>
                </div>
            }.into_any())
        >
            <div class="playground-body">
                <input
                    type="text"
                    class="hit-filter-input"
                    placeholder="Filter snippets…"
                    prop:value=move || hit_filter.get()
                    on:input=move |ev| hit_filter.set(event_target_value(&ev))
                />

                <HitList
                    hits=hits
                    query_text=query_text
                    profile_id=profile_id
                    hit_filter=hit_filter
                    highlighted_hit=highlighted_hit
                    mark_kind_for=mark_kind_for
                    cycle_mark=cycle_mark
                />
            </div>
        </Surface>
    }
    .into_any()
}

#[component]
fn HitList(
    hits: Vec<QueryHit>,
    query_text: String,
    profile_id: Uuid,
    hit_filter: RwSignal<String>,
    highlighted_hit: RwSignal<Option<usize>>,
    mark_kind_for: impl Fn(&str, Uuid) -> Option<MarkKind> + Copy + Send + Sync + 'static,
    cycle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let hits = StoredValue::new(hits);

    view! {
        <div class="playground-hits">
            {move || {
                let filter = hit_filter.get();
                let filter_lower = filter.to_lowercase();
                let highlighted = highlighted_hit.get();
                hits.with_value(|all| {
                    all.iter().enumerate().filter_map(|(i, hit)| {
                        if !filter_lower.is_empty() && !hit_matches_filter(hit, &filter_lower) {
                            return None;
                        }
                        let query_for_card = query_text.clone();
                        let toggle = cycle_mark.clone();
                        let is_highlighted = highlighted == Some(i);
                        Some(view! {
                            <RetrieveHitCard
                                rank=i+1
                                hit=hit.clone()
                                query_text=query_for_card
                                profile_id=profile_id
                                is_highlighted=is_highlighted
                                mark_kind_for=mark_kind_for
                                cycle_mark=toggle
                            />
                        })
                    }).collect_view()
                })
            }}
        </div>
    }
}

fn hit_matches_filter(hit: &QueryHit, filter_lower: &str) -> bool {
    if hit.snippet.to_lowercase().contains(filter_lower) {
        return true;
    }
    if let Some(title) = hit.document_title.as_ref() {
        if title.to_lowercase().contains(filter_lower) {
            return true;
        }
    }
    if let Some(heading) = hit.heading.as_ref() {
        if heading.to_lowercase().contains(filter_lower) {
            return true;
        }
    }
    false
}

#[component]
fn RetrieveHitCard(
    rank: usize,
    hit: QueryHit,
    query_text: String,
    profile_id: Uuid,
    is_highlighted: bool,
    mark_kind_for: impl Fn(&str, Uuid) -> Option<MarkKind> + Copy + Send + Sync + 'static,
    cycle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let chunk_id = hit.chunk_id;
    let query_for_check = query_text.clone();
    let mark_kind =
        Memo::new(move |_| chunk_id.and_then(|cid| mark_kind_for(&query_for_check, cid)));
    let query_for_toggle = query_text.clone();
    let toggle = move |_| {
        if let Some(cid) = chunk_id {
            cycle_mark(query_for_toggle.clone(), cid);
        }
    };

    let source = hit_source_link(&hit);
    let title = hit_display_title(&hit);
    let heading = hit.heading.clone();
    let snippet = hit.snippet.clone();
    let score = hit.score;
    let vector_id = hit.id.clone();
    let detail_chunk = hit.chunk_id.map(|c| c.to_string());
    let detail_doc = hit.document_id.map(|d| d.to_string());
    let detail_range = match (hit.char_start, hit.char_end) {
        (Some(s), Some(e)) => Some(format!("{s}..{e}")),
        _ => None,
    };

    let mark_button_class = move || match mark_kind.get() {
        Some(MarkKind::Relevant) => "playground-mark-btn playground-mark-btn-relevant",
        Some(MarkKind::Irrelevant) => "playground-mark-btn playground-mark-btn-irrelevant",
        None => "playground-mark-btn",
    };
    let mark_button_label = move || match mark_kind.get() {
        Some(MarkKind::Relevant) => "↑ Relevant",
        Some(MarkKind::Irrelevant) => "↓ Irrelevant",
        None => "Mark",
    };
    let mark_button_title =
        "Click to cycle: none → relevant → irrelevant → none. r / i to set directly.";

    let ask_in_chat_href = format!(
        "/playground/chat?q={}&profile={}",
        urlencoding::encode(&query_text),
        profile_id,
    );
    let query_for_card = query_text.clone();

    let header_actions: Children = Box::new(move || {
        view! {
            {source.clone().map(|href| view! {
                <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
            })}
            <a class="btn btn-ghost btn-sm" title="Ask this question in Chat" href=ask_in_chat_href>
                "Ask in Chat"
            </a>
            {chunk_id.map(|_| view! {
                <button
                    type="button"
                    class=mark_button_class
                    title=mark_button_title
                    on:click=toggle
                >
                    {mark_button_label}
                </button>
            })}
        }
        .into_any()
    });

    let details: Children = Box::new(move || {
        view! {
            <dl class="playground-hit-details">
                <dt>"vector id"</dt><dd>{vector_id}</dd>
                {detail_chunk.map(|c| view! { <><dt>"chunk_id"</dt><dd>{c}</dd></> })}
                {detail_doc.map(|d| view! { <><dt>"document_id"</dt><dd>{d}</dd></> })}
                {detail_range.map(|r| view! { <><dt>"range"</dt><dd>{r}</dd></> })}
            </dl>
        }
        .into_any()
    });

    view! {
        <HitCard
            rank=rank
            score=score
            title=title
            heading=heading
            snippet=snippet
            query=Some(query_for_card)
            badge=None
            header_actions=Some(header_actions)
            footer_actions=None
            details=Some(details)
            highlighted=is_highlighted
            extra_class=None
        />
    }
}
