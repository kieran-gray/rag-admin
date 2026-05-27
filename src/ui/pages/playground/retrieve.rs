use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::configuration::get_retrieval_profiles;
use crate::server_functions::query::query_documents;
use crate::shared::contracts::{
    MetadataFilterDto, QueryHit, QueryRequest, QueryResult, RetrievalProfileDto, Timings,
};
use crate::ui::components::playground::metadata_filters::metadata_filters_from_signal;
use crate::ui::components::playground::{
    LatencyBadge, QueryInput, RequestInspector, RetrievalControls,
};
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

#[derive(Clone)]
struct HistoryEntry {
    query: String,
    retrieval_profile_id: Uuid,
    top_k: u32,
    min_score: f32,
    filters: Vec<MetadataFilterDto>,
    result: Option<QueryResult>,
    error: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct MarkedRelevant {
    query: String,
    chunk_id: Uuid,
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
    let initial_retrieval_profile = retrieval_profiles
        .first()
        .map(|p| p.retrieval_profile_id)
        .unwrap_or_default();

    let query = RwSignal::new(String::new());
    let (retrieval_profile_id, set_retrieval_profile_id) = signal(initial_retrieval_profile);
    let top_k = RwSignal::new(8u32);
    let min_score = RwSignal::new(0.4f32);
    let filters = RwSignal::new(Vec::<MetadataFilterDto>::new());
    let busy = RwSignal::new(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let result = RwSignal::new(None::<QueryResult>);
    let history = RwSignal::new(Vec::<HistoryEntry>::new());
    let marked = RwSignal::new(Vec::<MarkedRelevant>::new());
    let last_request_json = RwSignal::new(String::new());

    let run_query =
        move |q: String, pid: Uuid, k: u32, m: f32, active_filters: Vec<MetadataFilterDto>| {
            if busy.get_untracked() || q.trim().is_empty() {
                return;
            }
            busy.set(true);
            set_error.set(None);
            let req = QueryRequest {
                retrieval_profile_id: pid,
                query: q.clone(),
                top_k: k,
                min_score: m,
                document_id: None,
                metadata_filters: active_filters.clone(),
            };
            last_request_json.set(serde_json::to_string_pretty(&req).unwrap_or_default());
            spawn_local(async move {
                match query_documents(req).await {
                    Ok(res) => {
                        let entry = HistoryEntry {
                            query: q.clone(),
                            retrieval_profile_id: pid,
                            top_k: k,
                            min_score: m,
                            filters: active_filters.clone(),
                            result: Some(res.clone()),
                            error: None,
                        };
                        history.update(|h| {
                            h.retain(|e| {
                                !(e.query == entry.query
                                    && e.retrieval_profile_id == entry.retrieval_profile_id)
                            });
                            h.insert(0, entry);
                            h.truncate(20);
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
                                    top_k: k,
                                    min_score: m,
                                    filters: active_filters,
                                    result: None,
                                    error: Some(e.to_string()),
                                },
                            );
                            h.truncate(20);
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

    let toggle_mark = move |query_text: String, chunk_id: Uuid| {
        marked.update(|m| {
            let key = MarkedRelevant {
                query: query_text,
                chunk_id,
            };
            if let Some(idx) = m.iter().position(|e| *e == key) {
                m.remove(idx);
            } else {
                m.push(key);
            }
        });
    };
    let is_marked = move |query_text: &str, chunk_id: Uuid| {
        marked.with(|m| {
            m.iter()
                .any(|e| e.query == query_text && e.chunk_id == chunk_id)
        })
    };

    let request_body_signal = Signal::derive(move || last_request_json.get());
    let has_request = Signal::derive(move || last_request_json.with(|s| !s.is_empty()));

    view! {
        <div class="playground-grid">
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

                {move || result.get().map(|r| {
                    let query_text = r.query.clone();
                    view! {
                        <ResultsList
                            result=r
                            query_text=query_text
                            is_marked=is_marked
                            toggle_mark=toggle_mark
                        />
                    }
                })}
            </div>

            <aside class="playground-sidebar">
                <Surface title="History".to_string()>
                    {move || {
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
                                view! {
                                    <button
                                        type="button"
                                        class="playground-history-row"
                                        on:click=move |_| on_click(entry_for_click.clone())
                                    >
                                        <div class="playground-history-query">{entry.query.clone()}</div>
                                        <div class="playground-history-meta">
                                            {if has_err {
                                                "failed".to_string()
                                            } else {
                                                format!("{} hits · k={}", hit_count, entry.top_k)
                                            }}
                                        </div>
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </Surface>

                {move || {
                    let m = marked.get();
                    if m.is_empty() {
                        ().into_any()
                    } else {
                        view! {
                            <Surface title=format!("Marked relevant · {}", m.len())>
                                <ul class="playground-marked-list">
                                    {m.into_iter().map(|entry| view! {
                                        <li>
                                            <div class="playground-marked-query">{entry.query}</div>
                                            <div class="playground-marked-chunk muted">
                                                {entry.chunk_id.to_string()}
                                            </div>
                                        </li>
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
fn ResultsList(
    result: QueryResult,
    query_text: String,
    is_marked: impl Fn(&str, Uuid) -> bool + Copy + Send + Sync + 'static,
    toggle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
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
    view! {
        <Surface
            title=format!("Results · {count}")
            actions=Box::new(move || view! {
                <ResultsLatency timings=timings />
            }.into_any())
        >
            <div class="playground-hits">
                {hits.into_iter().enumerate().map(|(i, hit)| {
                    let query_for_mark = query_text.clone();
                    let toggle = toggle_mark.clone();
                    view! {
                        <HitCard
                            rank=i+1
                            hit=hit
                            query_text=query_for_mark
                            is_marked=is_marked
                            toggle_mark=toggle
                        />
                    }
                }).collect_view()}
            </div>
        </Surface>
    }
    .into_any()
}

#[component]
fn ResultsLatency(timings: Timings) -> impl IntoView {
    view! { <LatencyBadge timings=timings /> }
}

#[component]
fn HitCard(
    rank: usize,
    hit: QueryHit,
    query_text: String,
    is_marked: impl Fn(&str, Uuid) -> bool + Copy + Send + Sync + 'static,
    toggle_mark: impl Fn(String, Uuid) + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let chunk_id = hit.chunk_id;
    let query_for_check = query_text.clone();
    let marked = Memo::new(move |_| {
        chunk_id
            .map(|cid| is_marked(&query_for_check, cid))
            .unwrap_or(false)
    });
    let query_for_toggle = query_text.clone();
    let toggle = move |_| {
        if let Some(cid) = chunk_id {
            toggle_mark(query_for_toggle.clone(), cid);
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

    view! {
        <article class="playground-hit">
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
                    {chunk_id.map(|_| view! {
                        <button
                            type="button"
                            class=move || if marked.get() { "btn btn-primary btn-sm" } else { "btn btn-sm" }
                            on:click=toggle
                        >
                            {move || if marked.get() { "Marked" } else { "Mark relevant" }}
                        </button>
                    })}
                </div>
            </header>
            <p class="playground-hit-snippet">{snippet}</p>
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
