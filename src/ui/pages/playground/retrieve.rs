use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::configuration::get_retrieval_profiles;
use crate::server_functions::query::query_documents;
use crate::shared::contracts::{
    MetadataFilterDto, QueryHit, QueryRequest, QueryResult, RetrievalProfileDto,
};
use crate::ui::components::playground::marks_store::{next_kind, Mark, MarkKind};
use crate::ui::components::playground::metadata_filters::metadata_filters_from_signal;
use crate::ui::components::playground::{
    marks_store, use_playground_context, HighlightedSnippet, LatencyBadge, QueryInput,
    RequestInspector, RetrievalControls, ScoreSparkline,
};
use leptos_router::hooks::use_query_map;
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

const HISTORY_LIMIT: usize = 20;
#[cfg(feature = "hydrate")]
const SUBMIT_QUERY_FOCUS_SELECTOR: &str = ".query-input-textarea";

#[derive(Clone)]
struct HistoryEntry {
    query: String,
    retrieval_profile_id: Uuid,
    profile_name: String,
    top_k: u32,
    min_score: f32,
    filters: Vec<MetadataFilterDto>,
    result: Option<QueryResult>,
    error: Option<String>,
    created_at_ms: f64,
}

#[derive(Clone)]
struct ComparisonRun {
    query: String,
    profile_name: String,
    top_k: u32,
    result: QueryResult,
}

#[derive(Clone, Copy)]
enum CompareColumn {
    Baseline,
    Candidate,
}

#[derive(Clone, Copy)]
enum RankDelta {
    Up(usize),
    Down(usize),
    Same,
    NotInOther,
    Unknown,
}

struct CompareSummary {
    moved_up: usize,
    moved_down: usize,
    same: usize,
    new_count: usize,
    dropped: usize,
    jaccard: f32,
}

fn build_ranks(hits: &[QueryHit]) -> HashMap<Uuid, usize> {
    let mut m = HashMap::with_capacity(hits.len());
    for (i, hit) in hits.iter().enumerate() {
        if let Some(cid) = hit.chunk_id {
            m.entry(cid).or_insert(i + 1);
        }
    }
    m
}

fn compute_delta(this_rank: usize, hit: &QueryHit, other_ranks: &HashMap<Uuid, usize>) -> RankDelta {
    let Some(cid) = hit.chunk_id else {
        return RankDelta::Unknown;
    };
    match other_ranks.get(&cid) {
        Some(&other) if other > this_rank => RankDelta::Up(other - this_rank),
        Some(&other) if other < this_rank => RankDelta::Down(this_rank - other),
        Some(_) => RankDelta::Same,
        None => RankDelta::NotInOther,
    }
}

fn delta_badge(delta: RankDelta, col: CompareColumn) -> Option<(String, &'static str)> {
    match (delta, col) {
        (RankDelta::Up(n), _) => Some((format!("↑{n}"), "compare-delta compare-delta-up")),
        (RankDelta::Down(n), _) => Some((format!("↓{n}"), "compare-delta compare-delta-down")),
        (RankDelta::Same, _) => Some(("=".to_string(), "compare-delta compare-delta-same")),
        (RankDelta::NotInOther, CompareColumn::Baseline) => {
            Some(("OUT".to_string(), "compare-delta compare-delta-out"))
        }
        (RankDelta::NotInOther, CompareColumn::Candidate) => {
            Some(("NEW".to_string(), "compare-delta compare-delta-new"))
        }
        (RankDelta::Unknown, _) => None,
    }
}

fn compare_summary(baseline: &[QueryHit], candidate: &[QueryHit]) -> CompareSummary {
    let baseline_ids: HashSet<Uuid> = baseline.iter().filter_map(|h| h.chunk_id).collect();
    let candidate_ids: HashSet<Uuid> = candidate.iter().filter_map(|h| h.chunk_id).collect();
    let intersection = baseline_ids.intersection(&candidate_ids).count();
    let union = baseline_ids.union(&candidate_ids).count();
    #[allow(clippy::cast_precision_loss)]
    let jaccard = if union == 0 {
        1.0
    } else {
        intersection as f32 / union as f32
    };

    let b_ranks = build_ranks(baseline);
    let c_ranks = build_ranks(candidate);
    let mut moved_up = 0;
    let mut moved_down = 0;
    let mut same = 0;
    for cid in baseline_ids.intersection(&candidate_ids) {
        let Some(&b) = b_ranks.get(cid) else { continue };
        let Some(&c) = c_ranks.get(cid) else { continue };
        if c < b {
            moved_up += 1;
        } else if c > b {
            moved_down += 1;
        } else {
            same += 1;
        }
    }
    let new_count = candidate_ids.difference(&baseline_ids).count();
    let dropped = baseline_ids.difference(&candidate_ids).count();

    CompareSummary {
        moved_up,
        moved_down,
        same,
        new_count,
        dropped,
        jaccard,
    }
}

#[component]
pub fn RetrievePage() -> impl IntoView {
    let retrieval_profiles = Resource::new(
        || (),
        |_| async move { get_retrieval_profiles().await.unwrap_or_default() },
    );

    view! {
        <div>
            <PageHeader
                title="Retrieve"
                subtitle="Run retrieval against any retrieval profile. Results route back to the source document for chunking iteration.".to_string()
            />
            <Transition fallback=|| view! { <Surface><p class="muted">"Loading…"</p></Surface> }>
                {move || retrieval_profiles.get().map(|profiles| {
                    if profiles.is_empty() {
                        view! {
                            <Surface>
                                <EmptyState
                                    title="No retrieval profiles configured"
                                    body="Create a retrieval profile on the Profiles page before running queries.".to_string()
                                />
                            </Surface>
                        }.into_any()
                    } else {
                        view! { <PlaygroundBody retrieval_profiles=profiles /> }.into_any()
                    }
                })}
            </Transition>
        </div>
    }
}

#[component]
fn PlaygroundBody(retrieval_profiles: Vec<RetrievalProfileDto>) -> impl IntoView {
    let retrieval_profiles_stored = StoredValue::new(retrieval_profiles.clone());
    let playground = use_playground_context();
    let query_params = use_query_map();
    let url_query = query_params.with(|q| q.get("q").unwrap_or_default().to_string());
    let url_profile_param = query_params.with(|q| q.get("profile").unwrap_or_default().to_string());
    let url_profile_uuid = Uuid::parse_str(&url_profile_param).ok();

    let initial_query = if url_query.is_empty() {
        playground.last_query.get_untracked()
    } else {
        url_query
    };
    let initial_retrieval_profile = url_profile_uuid
        .filter(|id| {
            retrieval_profiles
                .iter()
                .any(|p| p.retrieval_profile_id == *id)
        })
        .or_else(|| {
            playground.last_profile.get_untracked().filter(|id| {
                retrieval_profiles
                    .iter()
                    .any(|p| p.retrieval_profile_id == *id)
            })
        })
        .or_else(|| retrieval_profiles.first().map(|p| p.retrieval_profile_id))
        .unwrap_or_default();

    let query = RwSignal::new(initial_query);
    let (retrieval_profile_id, set_retrieval_profile_id) = signal(initial_retrieval_profile);
    let top_k = RwSignal::new(8u32);
    let min_score = RwSignal::new(0.4f32);
    let filters = RwSignal::new(Vec::<MetadataFilterDto>::new());
    let busy = RwSignal::new(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let result = RwSignal::new(None::<QueryResult>);
    let history = RwSignal::new(Vec::<HistoryEntry>::new());
    let marks = RwSignal::new(Vec::<Mark>::new());
    let hit_filter = RwSignal::new(String::new());
    let highlighted_hit = RwSignal::new(None::<usize>);
    let last_request_json = RwSignal::new(String::new());
    let baseline = RwSignal::new(None::<ComparisonRun>);

    let profile_name_for = move |pid: Uuid| {
        retrieval_profiles_stored.with_value(|profiles| {
            profiles
                .iter()
                .find(|p| p.retrieval_profile_id == pid)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Unknown profile".to_string())
        })
    };

    Effect::new(move |_| {
        let pid = retrieval_profile_id.get();
        let loaded = marks_store::load(pid);
        marks.set(loaded);
    });

    Effect::new(move |_| {
        let pid = retrieval_profile_id.get_untracked();
        marks.with(|m| marks_store::save(pid, m));
    });

    let run_query =
        move |q: String, pid: Uuid, k: u32, m: f32, active_filters: Vec<MetadataFilterDto>| {
            if busy.get_untracked() || q.trim().is_empty() {
                return;
            }
            playground.last_query.set(q.clone());
            playground.last_profile.set(Some(pid));
            playground.last_filters.set(active_filters.clone());
            busy.set(true);
            set_error.set(None);
            hit_filter.set(String::new());
            highlighted_hit.set(None);
            let req = QueryRequest {
                retrieval_profile_id: pid,
                query: q.clone(),
                top_k: k,
                min_score: m,
                document_id: None,
                metadata_filters: active_filters.clone(),
            };
            last_request_json.set(serde_json::to_string_pretty(&req).unwrap_or_default());
            let profile_name = profile_name_for(pid);
            let created_at_ms = now_ms();
            spawn_local(async move {
                match query_documents(req).await {
                    Ok(res) => {
                        let entry = HistoryEntry {
                            query: q.clone(),
                            retrieval_profile_id: pid,
                            profile_name,
                            top_k: k,
                            min_score: m,
                            filters: active_filters.clone(),
                            result: Some(res.clone()),
                            error: None,
                            created_at_ms,
                        };
                        history.update(|h| {
                            h.retain(|e| {
                                !(e.query == entry.query
                                    && e.retrieval_profile_id == entry.retrieval_profile_id)
                            });
                            h.insert(0, entry);
                            h.truncate(HISTORY_LIMIT);
                        });
                        result.set(Some(res));
                    }
                    Err(e) => {
                        history.update(|h| {
                            h.insert(
                                0,
                                HistoryEntry {
                                    query: q,
                                    retrieval_profile_id: pid,
                                    profile_name,
                                    top_k: k,
                                    min_score: m,
                                    filters: active_filters,
                                    result: None,
                                    error: Some(e.to_string()),
                                    created_at_ms,
                                },
                            );
                            h.truncate(HISTORY_LIMIT);
                        });
                        set_error.set(Some(e.to_string()));
                    }
                }
                busy.set(false);
            });
        };

    let submit = Callback::new(move |()| {
        run_query(
            query.get_untracked(),
            retrieval_profile_id.get_untracked(),
            top_k.get_untracked(),
            min_score.get_untracked(),
            metadata_filters_from_signal(filters),
        );
    });

    let pin_baseline = move || {
        let Some(res) = result.get_untracked() else {
            return;
        };
        let entry = history.with_untracked(|h| h.first().cloned());
        let Some(entry) = entry else {
            return;
        };
        baseline.set(Some(ComparisonRun {
            query: entry.query,
            profile_name: entry.profile_name,
            top_k: entry.top_k,
            result: res,
        }));
    };

    let unpin_baseline = move || {
        baseline.set(None);
    };

    let on_history_click = move |entry: HistoryEntry| {
        query.set(entry.query.clone());
        set_retrieval_profile_id.set(entry.retrieval_profile_id);
        top_k.set(entry.top_k);
        min_score.set(entry.min_score);
        filters.set(entry.filters.clone());
        run_query(
            entry.query,
            entry.retrieval_profile_id,
            entry.top_k,
            entry.min_score,
            entry.filters,
        );
    };

    let cycle_mark = move |query_text: String, chunk_id: Uuid| {
        marks.update(|m| {
            let existing = m
                .iter()
                .position(|e| e.query == query_text && e.chunk_id == chunk_id);
            let current_kind = existing.and_then(|i| m.get(i).map(|x| x.kind));
            let next = next_kind(current_kind);
            match (existing, next) {
                (Some(i), Some(k)) => {
                    if let Some(entry) = m.get_mut(i) {
                        entry.kind = k;
                    }
                }
                (Some(i), None) => {
                    m.remove(i);
                }
                (None, Some(k)) => m.push(Mark {
                    query: query_text,
                    chunk_id,
                    kind: k,
                }),
                (None, None) => {}
            }
        });
    };
    let mark_kind_for = move |query_text: &str, chunk_id: Uuid| -> Option<MarkKind> {
        marks.with(|m| {
            m.iter()
                .find(|e| e.query == query_text && e.chunk_id == chunk_id)
                .map(|e| e.kind)
        })
    };

    let on_keydown = move |ev: KeyboardEvent| {
        if !is_global_target(&ev) {
            return;
        }
        match ev.key().as_str() {
            "/" => {
                ev.prevent_default();
                focus_query_textarea();
            }
            "j" => {
                ev.prevent_default();
                cycle_highlighted_hit(highlighted_hit, result, 1);
            }
            "k" => {
                ev.prevent_default();
                cycle_highlighted_hit(highlighted_hit, result, -1);
            }
            "r" => {
                if let Some((q, cid)) = highlighted_hit_chunk(highlighted_hit, result) {
                    ev.prevent_default();
                    set_mark_to(marks, q, cid, MarkKind::Relevant);
                }
            }
            "i" => {
                if let Some((q, cid)) = highlighted_hit_chunk(highlighted_hit, result) {
                    ev.prevent_default();
                    set_mark_to(marks, q, cid, MarkKind::Irrelevant);
                }
            }
            _ => {}
        }
    };

    let request_body_signal = Signal::derive(move || last_request_json.get());
    let has_request = Signal::derive(move || last_request_json.with(|s| !s.is_empty()));

    view! {
        <div class="playground-grid" on:keydown=on_keydown>
            <div class="playground-main">
                <Surface title="Query".to_string() actions=Box::new(move || view! {
                    <RetrievalProfilePicker
                        profiles=retrieval_profiles_stored.get_value()
                        value=retrieval_profile_id
                        set_value=set_retrieval_profile_id
                    />
                }.into_any())>
                    <div class="playground-body">
                        <QueryInput
                            value=query
                            busy=Signal::from(busy)
                            on_submit=submit
                            placeholder="Ask a question…".to_string()
                            submit_label="Run query".to_string()
                            busy_label="Querying…".to_string()
                        />

                        <RetrievalControls top_k=top_k min_score=min_score filters=filters />

                        {move || error.get().map(|e| view! {
                            <div class="log-line-error">{e}</div>
                        })}

                        {move || has_request.get().then(|| view! {
                            <RequestInspector body=request_body_signal label="Inspect last request".to_string() />
                        })}
                    </div>
                </Surface>

                {move || {
                    let current = result.get();
                    let base = baseline.get();
                    match (base, current) {
                        (Some(b), Some(c)) => view! {
                            <CompareView
                                baseline=b
                                candidate_query=c.query.clone()
                                candidate_hits=c.hits.clone()
                                on_unpin=unpin_baseline
                            />
                        }.into_any(),
                        (None, Some(c)) => {
                            let query_text = c.query.clone();
                            let min = c.hits.iter().map(|h| h.score).fold(1.0_f32, f32::min);
                            let threshold = min_score.get_untracked().min(min);
                            let active_profile = c.retrieval_profile_id;
                            view! {
                                <ResultsPanel
                                    result=c
                                    query_text=query_text
                                    profile_id=active_profile
                                    min_score=threshold
                                    hit_filter=hit_filter
                                    highlighted_hit=highlighted_hit
                                    mark_kind_for=mark_kind_for
                                    cycle_mark=cycle_mark
                                    on_pin_baseline=pin_baseline
                                />
                            }.into_any()
                        }
                        _ => ().into_any(),
                    }
                }}
            </div>

            <aside class="playground-sidebar">
                <Surface title="History".to_string()>
                    {move || {
                        let now = now_ms();
                        let entries = history.get();
                        if entries.is_empty() {
                            view! {
                                <p class="muted text-sm">"No queries yet."</p>
                            }.into_any()
                        } else {
                            entries.into_iter().map(|entry| {
                                let on_click = on_history_click;
                                let entry_for_click = entry.clone();
                                let hit_count = entry.result.as_ref().map(|r| r.hits.len()).unwrap_or(0);
                                let has_err = entry.error.is_some();
                                let relative = relative_time(now, entry.created_at_ms);
                                let filter_count = entry.filters.len();
                                let summary = if has_err {
                                    "failed".to_string()
                                } else if filter_count > 0 {
                                    let mut s = format!("{hit_count} hits · k={}", entry.top_k);
                                    _ = write!(s, " · {filter_count}f");
                                    s
                                } else {
                                    format!("{hit_count} hits · k={}", entry.top_k)
                                };
                                view! {
                                    <button
                                        type="button"
                                        class="playground-history-row"
                                        on:click=move |_| on_click(entry_for_click.clone())
                                    >
                                        <div class="playground-history-query">{entry.query.clone()}</div>
                                        <div class="playground-history-meta">
                                            <span>{entry.profile_name.clone()}</span>
                                            <span class="playground-history-dot">"·"</span>
                                            <span>{summary}</span>
                                            <span class="playground-history-dot">"·"</span>
                                            <span class="faint">{relative}</span>
                                        </div>
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </Surface>

                {move || {
                    let m = marks.get();
                    if m.is_empty() {
                        ().into_any()
                    } else {
                        let relevant = m.iter().filter(|x| x.kind == MarkKind::Relevant).count();
                        let irrelevant = m.iter().filter(|x| x.kind == MarkKind::Irrelevant).count();
                        view! {
                            <Surface title=format!("Marks · {} ↑ · {} ↓", relevant, irrelevant)>
                                <ul class="playground-marked-list">
                                    {m.into_iter().map(|entry| {
                                        let icon = match entry.kind {
                                            MarkKind::Relevant => "↑",
                                            MarkKind::Irrelevant => "↓",
                                        };
                                        view! {
                                            <li>
                                                <div class="playground-marked-query">
                                                    <span class="playground-mark-icon">{icon}</span>
                                                    {entry.query}
                                                </div>
                                                <div class="playground-marked-chunk muted">
                                                    {entry.chunk_id.to_string()}
                                                </div>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            </Surface>
                        }.into_any()
                    }
                }}
            </aside>
        </div>
    }
}

fn cycle_highlighted_hit(
    highlighted_hit: RwSignal<Option<usize>>,
    result: RwSignal<Option<QueryResult>>,
    delta: i32,
) {
    let count = result.with(|r| r.as_ref().map_or(0, |x| x.hits.len()));
    if count == 0 {
        return;
    }
    highlighted_hit.update(|h| {
        let next = match *h {
            None => {
                if delta > 0 {
                    0
                } else {
                    count - 1
                }
            }
            Some(i) => {
                let i = i as i32 + delta;
                let n = count as i32;
                ((i % n + n) % n) as usize
            }
        };
        *h = Some(next);
    });
}

fn highlighted_hit_chunk(
    highlighted_hit: RwSignal<Option<usize>>,
    result: RwSignal<Option<QueryResult>>,
) -> Option<(String, Uuid)> {
    let idx = highlighted_hit.get_untracked()?;
    result.with_untracked(|r| {
        let res = r.as_ref()?;
        let hit = res.hits.get(idx)?;
        Some((res.query.clone(), hit.chunk_id?))
    })
}

fn set_mark_to(marks: RwSignal<Vec<Mark>>, query: String, chunk_id: Uuid, kind: MarkKind) {
    marks.update(|m| {
        let position = m
            .iter()
            .position(|e| e.query == query && e.chunk_id == chunk_id);
        match position {
            Some(idx) => {
                let same_kind = m.get(idx).is_some_and(|e| e.kind == kind);
                if same_kind {
                    m.remove(idx);
                } else if let Some(entry) = m.get_mut(idx) {
                    entry.kind = kind;
                }
            }
            None => m.push(Mark {
                query,
                chunk_id,
                kind,
            }),
        }
    });
}

fn is_global_target(ev: &KeyboardEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let Some(target) = ev.target() else {
        return true;
    };
    let Ok(el) = target.dyn_into::<web_sys::Element>() else {
        return true;
    };
    let tag = el.tag_name().to_lowercase();
    !matches!(tag.as_str(), "input" | "textarea" | "select")
}

#[cfg(feature = "hydrate")]
fn focus_query_textarea() {
    use leptos::wasm_bindgen::JsCast;
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(Some(el)) = document.query_selector(SUBMIT_QUERY_FOCUS_SELECTOR) else {
        return;
    };
    if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
        drop(html.focus());
    }
}

#[cfg(not(feature = "hydrate"))]
fn focus_query_textarea() {}

#[cfg(feature = "hydrate")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(feature = "hydrate"))]
fn now_ms() -> f64 {
    0.0
}

fn relative_time(now_ms: f64, then_ms: f64) -> String {
    if then_ms <= 0.0 || now_ms <= 0.0 {
        return String::new();
    }
    let diff = ((now_ms - then_ms).max(0.0) / 1000.0) as u64;
    if diff < 5 {
        return "just now".to_string();
    }
    if diff < 60 {
        return format!("{diff}s ago");
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86_400 {
        return format!("{}h ago", diff / 3600);
    }
    format!("{}d ago", diff / 86_400)
}

#[component]
fn RetrievalProfilePicker(
    profiles: Vec<RetrievalProfileDto>,
    value: ReadSignal<Uuid>,
    set_value: WriteSignal<Uuid>,
) -> impl IntoView {
    view! {
        <label class="playground-picker-label">
            <span class="eyebrow">"Retrieval profile"</span>
            <select
                class="playground-profile-picker"
                on:change=move |ev| {
                    if let Ok(uuid) = Uuid::parse_str(&event_target_value(&ev)) {
                        set_value.set(uuid);
                    }
                }
            >
                {profiles.into_iter().map(|p| {
                    let pid = p.retrieval_profile_id;
                    let id_str = pid.to_string();
                    view! {
                        <option value=id_str.clone() selected=move || value.get() == pid>
                            {p.name}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn ResultsPanel(
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
fn CompareView(
    baseline: ComparisonRun,
    candidate_query: String,
    candidate_hits: Vec<QueryHit>,
    on_unpin: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let baseline_hits = baseline.result.hits.clone();
    let summary = compare_summary(&baseline_hits, &candidate_hits);

    let baseline_ranks = build_ranks(&baseline_hits);
    let candidate_ranks = build_ranks(&candidate_hits);

    let jaccard_pct = format!("{:.0}%", summary.jaccard * 100.0);
    let CompareSummary {
        moved_up,
        moved_down,
        same,
        new_count,
        dropped,
        jaccard: _,
    } = summary;

    let baseline_label = format!(
        "Baseline · {} · k={}",
        baseline.profile_name, baseline.top_k
    );
    let baseline_query = baseline.query.clone();
    let candidate_label = format!("Candidate · {} hits", candidate_hits.len());

    view! {
        <Surface
            title="Comparison".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-sm btn-ghost"
                    on:click=move |_| on_unpin()
                >
                    "Unpin baseline"
                </button>
            }.into_any())
        >
            <div class="playground-body">
                <div class="compare-summary">
                    <span class="compare-summary-stat compare-summary-up">{format!("↑ {moved_up}")}</span>
                    <span class="compare-summary-stat compare-summary-down">{format!("↓ {moved_down}")}</span>
                    <span class="compare-summary-stat">{format!("= {same}")}</span>
                    <span class="compare-summary-stat compare-summary-new">{format!("NEW {new_count}")}</span>
                    <span class="compare-summary-stat compare-summary-out">{format!("OUT {dropped}")}</span>
                    <span class="compare-summary-stat compare-summary-jaccard">
                        "Jaccard "{jaccard_pct}
                    </span>
                </div>

                <div class="compare-grid">
                    <CompareColumnView
                        column=CompareColumn::Baseline
                        title=baseline_label
                        query=baseline_query.clone()
                        hits=baseline_hits
                        other_ranks=candidate_ranks
                    />
                    <CompareColumnView
                        column=CompareColumn::Candidate
                        title=candidate_label
                        query=candidate_query
                        hits=candidate_hits
                        other_ranks=baseline_ranks
                    />
                </div>
            </div>
        </Surface>
    }
}

#[component]
fn CompareColumnView(
    column: CompareColumn,
    title: String,
    query: String,
    hits: Vec<QueryHit>,
    other_ranks: HashMap<Uuid, usize>,
) -> impl IntoView {
    let other_ranks = StoredValue::new(other_ranks);

    view! {
        <div class="compare-column">
            <header class="compare-column-head">
                <div class="compare-column-title">{title}</div>
                <div class="compare-column-query muted text-sm">{query.clone()}</div>
            </header>
            <div class="compare-column-hits">
                {hits.into_iter().enumerate().map(|(i, hit)| {
                    let rank = i + 1;
                    let delta = other_ranks.with_value(|r| compute_delta(rank, &hit, r));
                    view! {
                        <CompareHitCard
                            rank=rank
                            hit=hit
                            query_text=query.clone()
                            delta=delta
                            column=column
                        />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn CompareHitCard(
    rank: usize,
    hit: QueryHit,
    query_text: String,
    delta: RankDelta,
    column: CompareColumn,
) -> impl IntoView {
    let title = hit
        .document_title
        .clone()
        .or_else(|| hit.source_ref_key.clone())
        .unwrap_or_else(|| "Unknown source".to_string());
    let heading = hit.heading.clone();
    let snippet = hit.snippet.clone();
    let score_display = format!("{:.3}", hit.score);
    let badge = delta_badge(delta, column);

    let source_link = match (hit.document_id, hit.char_start, hit.char_end) {
        (Some(doc_id), Some(start), Some(end)) => Some(format!(
            "/documents/by-id/{doc_id}?tab=source&ref_start={start}&ref_end={end}"
        )),
        (Some(doc_id), _, _) => Some(format!("/documents/by-id/{doc_id}?tab=source")),
        _ => None,
    };

    view! {
        <article class="playground-hit compare-hit">
            <header class="playground-hit-head">
                <span class="playground-hit-rank">{rank}</span>
                <span class="playground-hit-score">{score_display}</span>
                <div class="playground-hit-title">
                    <div class="playground-hit-doc">{title}</div>
                    {heading.map(|h| view! { <div class="playground-hit-heading muted">{h}</div> })}
                </div>
                {badge.map(|(label, class)| view! {
                    <span class=class>{label}</span>
                })}
            </header>
            <HighlightedSnippet text=snippet query=query_text />
            {source_link.map(|href| view! {
                <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
            })}
        </article>
    }
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
                            <HitCard
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
fn HitCard(
    rank: usize,
    hit: QueryHit,
    query_text: String,
    profile_id: Uuid,
    is_highlighted: bool,
    mark_kind_for: impl Fn(&str, Uuid) -> Option<MarkKind> + Copy + Send + Sync + 'static,
    cycle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let chunk_id = hit.chunk_id;
    let query_for_check = query_text.clone();
    let mark_kind = Memo::new(move |_| {
        chunk_id.and_then(|cid| mark_kind_for(&query_for_check, cid))
    });
    let query_for_toggle = query_text.clone();
    let toggle = move |_| {
        if let Some(cid) = chunk_id {
            cycle_mark(query_for_toggle.clone(), cid);
        }
    };

    let source_link = match (hit.document_id, hit.char_start, hit.char_end) {
        (Some(doc_id), Some(start), Some(end)) => Some(format!(
            "/documents/by-id/{doc_id}?tab=source&ref_start={start}&ref_end={end}"
        )),
        (Some(doc_id), _, _) => Some(format!("/documents/by-id/{doc_id}?tab=source")),
        _ => None,
    };

    let title = hit
        .document_title
        .clone()
        .or_else(|| hit.source_ref_key.clone())
        .unwrap_or_else(|| "Unknown source".to_string());
    let heading = hit.heading.clone();
    let snippet = hit.snippet.clone();
    let score_display = format!("{:.3}", hit.score);

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

    let card_class = if is_highlighted {
        "playground-hit playground-hit-highlighted"
    } else {
        "playground-hit"
    };

    view! {
        <article class=card_class>
            <header class="playground-hit-head">
                <span class="playground-hit-rank">{rank}</span>
                <span class="playground-hit-score">{score_display}</span>
                <div class="playground-hit-title">
                    <div class="playground-hit-doc">{title}</div>
                    {heading.map(|h| view! { <div class="playground-hit-heading muted">{h}</div> })}
                </div>
                <div class="playground-hit-actions">
                    {source_link.clone().map(|href| view! {
                        <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
                    })}
                    <a
                        class="btn btn-ghost btn-sm"
                        title="Ask this question in Chat"
                        href=format!(
                            "/playground/chat?q={}&profile={}",
                            urlencoding::encode(&query_text),
                            profile_id,
                        )
                    >
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
                </div>
            </header>
            <HighlightedSnippet text=snippet query=query_text.clone() />
            <button
                type="button"
                class="playground-hit-reveal"
                on:click=move |_| set_expanded.update(|v| *v = !*v)
            >
                {move || if expanded.get() { "Hide details" } else { "Details" }}
            </button>
            {move || if expanded.get() {
                let id_clone = hit.id.clone();
                let chunk = hit.chunk_id.map(|c| c.to_string());
                let doc = hit.document_id.map(|d| d.to_string());
                let range = match (hit.char_start, hit.char_end) {
                    (Some(s), Some(e)) => Some(format!("{s}..{e}")),
                    _ => None,
                };
                view! {
                    <dl class="playground-hit-details">
                        <dt>"vector id"</dt><dd>{id_clone}</dd>
                        {chunk.map(|c| view! { <><dt>"chunk_id"</dt><dd>{c}</dd></> })}
                        {doc.map(|d| view! { <><dt>"document_id"</dt><dd>{d}</dd></> })}
                        {range.map(|r| view! { <><dt>"range"</dt><dd>{r}</dd></> })}
                    </dl>
                }.into_any()
            } else {
                ().into_any()
            }}
        </article>
    }
}
