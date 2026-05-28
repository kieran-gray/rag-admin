use std::fmt::Write as _;

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;
use uuid::Uuid;

use crate::server_functions::configuration::get_retrieval_profiles;
use crate::server_functions::query::query_documents;
use crate::shared::contracts::{MetadataFilterDto, QueryRequest, QueryResult, RetrievalProfileDto};
use crate::ui::components::playground::marks_store::{next_kind, Mark, MarkKind};
use crate::ui::components::playground::metadata_filters::metadata_filters_from_signal;
use crate::ui::components::playground::{
    marks_store, use_playground_context, QueryInput, RequestInspector, RetrievalControls,
    RetrievalProfilePicker,
};
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

use super::compare::{CompareView, ComparisonRun};
use super::history::{now_ms, relative_time, HistoryEntry};
use super::keyboard::{
    cycle_highlighted_hit, focus_query_textarea, highlighted_hit_chunk, is_global_target,
    set_mark_to,
};
use super::results::ResultsPanel;

const HISTORY_LIMIT: usize = 20;

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
    let url_query = query_params.with_untracked(|q| q.get("q").unwrap_or_default().to_string());
    let url_profile_param = query_params.with_untracked(|q| q.get("profile").unwrap_or_default().to_string());
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
