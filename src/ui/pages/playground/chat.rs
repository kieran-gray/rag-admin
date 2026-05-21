use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::chat::chat_query;
use crate::server_functions::configuration::get_retrieval_profiles;
use crate::shared::contracts::{
    ChatRequest, ChatResponse, MetadataFilterDto, QueryHit, RetrievalProfileDto,
};
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

#[derive(Clone)]
struct ChatTurn {
    question: String,
    response: Option<ChatResponse>,
    error: Option<String>,
}

#[component]
pub fn ChatPage() -> impl IntoView {
    let retrieval_profiles = Resource::new(
        || (),
        |_| async move { get_retrieval_profiles().await.unwrap_or_default() },
    );

    view! {
        <div>
            <PageHeader
                title="Chat"
                subtitle="Ask a question. Retrieves top-K chunks, then prompts the retrieval profile's generation model with the same format used in production.".to_string()
            />
            <Transition fallback=|| view! { <Surface><p class="muted">"Loading…"</p></Surface> }>
                {move || retrieval_profiles.get().map(|profiles| {
                    if profiles.is_empty() {
                        view! {
                            <Surface>
                                <EmptyState
                                    title="No retrieval profiles configured"
                                    body="Create a retrieval profile on the Profiles page before chatting.".to_string()
                                />
                            </Surface>
                        }.into_any()
                    } else {
                        view! { <ChatBody retrieval_profiles=profiles /> }.into_any()
                    }
                })}
            </Transition>
        </div>
    }
}

#[component]
fn ChatBody(retrieval_profiles: Vec<RetrievalProfileDto>) -> impl IntoView {
    let retrieval_profiles_stored = StoredValue::new(retrieval_profiles.clone());
    let initial_retrieval_profile = retrieval_profiles
        .first()
        .map(|p| p.retrieval_profile_id)
        .unwrap_or_default();

    let (question, set_question) = signal(String::new());
    let (retrieval_profile_id, set_retrieval_profile_id) = signal(initial_retrieval_profile);
    let (top_k, set_top_k) = signal::<u32>(8);
    let (min_score, set_min_score) = signal::<f32>(0.4);
    let (source_slug, set_source_slug) = signal(String::new());
    let (source_version, set_source_version) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (turns, set_turns) = signal::<Vec<ChatTurn>>(Vec::new());

    let submit = move |_| {
        let q = question.get();
        if busy.get_untracked() || q.trim().is_empty() {
            return;
        }
        let pid = retrieval_profile_id.get();
        let k = top_k.get();
        let m = min_score.get();
        let filters = build_metadata_filters(&source_slug.get(), &source_version.get());

        set_busy.set(true);
        set_error.set(None);
        set_question.set(String::new());

        spawn_local(async move {
            let req = ChatRequest {
                retrieval_profile_id: pid,
                query: q.clone(),
                top_k: k,
                min_score: m,
                metadata_filters: filters,
            };
            match chat_query(req).await {
                Ok(res) => {
                    set_turns.update(|t| {
                        t.push(ChatTurn {
                            question: q.clone(),
                            response: Some(res),
                            error: None,
                        });
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    set_turns.update(|t| {
                        t.push(ChatTurn {
                            question: q.clone(),
                            response: None,
                            error: Some(msg.clone()),
                        });
                    });
                    set_error.set(Some(msg));
                }
            }
            set_busy.set(false);
        });
    };

    let clear_history = move |_| {
        set_turns.set(Vec::new());
        set_error.set(None);
    };

    view! {
        <div class="playground-grid">
            <div class="playground-main">
                <Surface title="Conversation".to_string() actions=Box::new(move || view! {
                    <RetrievalProfilePicker
                        profiles=retrieval_profiles_stored.get_value()
                        value=retrieval_profile_id
                        set_value=set_retrieval_profile_id
                    />
                }.into_any())>
                    <ChatTranscript turns=turns busy=busy />

                    <textarea
                        class="playground-query-input mt-3"
                        placeholder="Ask a question…"
                        prop:value=move || question.get()
                        on:input=move |ev| set_question.set(event_target_value(&ev))
                        rows="8"
                    ></textarea>

                    <div class="playground-filters">
                        <label class="playground-control">
                            <span>"source_slug"</span>
                            <input
                                type="text"
                                placeholder="any"
                                prop:value=move || source_slug.get()
                                on:input=move |ev| set_source_slug.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="playground-control">
                            <span>"source_version"</span>
                            <input
                                type="text"
                                placeholder="any"
                                prop:value=move || source_version.get()
                                on:input=move |ev| set_source_version.set(event_target_value(&ev))
                            />
                        </label>
                    </div>
                    <div class="playground-controls">
                        <label class="playground-control">
                            <span>"Top-K"</span>
                            <input
                                type="number"
                                min="1"
                                max="50"
                                prop:value=move || top_k.get().to_string()
                                on:input=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                        set_top_k.set(v.clamp(1, 50));
                                    }
                                }
                            />
                        </label>
                        <label class="playground-control">
                            <span>"Min score"</span>
                            <input
                                type="number"
                                min="0"
                                max="1"
                                step="0.05"
                                prop:value=move || format!("{:.2}", min_score.get())
                                on:input=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                                        set_min_score.set(v.clamp(0.0, 1.0));
                                    }
                                }
                            />
                        </label>
                        <div class="playground-control-spacer"></div>
                        <button
                            type="button"
                            class="btn"
                            disabled=move || busy.get() || turns.with(Vec::is_empty)
                            on:click=clear_history
                        >
                            "Clear"
                        </button>
                        <button
                            type="button"
                            class="btn btn-primary"
                            disabled=move || busy.get() || question.with(|q| q.trim().is_empty())
                            on:click=submit
                        >
                            {move || if busy.get() { "Asking…" } else { "Send" }}
                        </button>
                    </div>

                    {move || error.get().map(|e| view! {
                        <div class="log-line-error mt-3">{e}</div>
                    })}
                </Surface>
            </div>

            <aside class="playground-sidebar">
                <Surface title="Sources".to_string()>
                    {move || {
                        let last = turns.with(|t| t.last().and_then(|t| t.response.clone()));
                        match last {
                            None => view! {
                                <p class="muted text-sm">"Ask a question to see the retrieved chunks here."</p>
                            }.into_any(),
                            Some(res) if res.hits.is_empty() => view! {
                                <p class="muted text-sm">"No chunks were retrieved for the last answer."</p>
                            }.into_any(),
                            Some(res) => view! {
                                <SourcesList hits=res.hits />
                            }.into_any(),
                        }
                    }}
                </Surface>
            </aside>
        </div>
    }
}

#[component]
fn ChatTranscript(turns: ReadSignal<Vec<ChatTurn>>, busy: ReadSignal<bool>) -> impl IntoView {
    view! {
        <div class="chat-transcript">
            {move || {
                let list = turns.get();
                if list.is_empty() && !busy.get() {
                    view! {
                        <p class="muted text-sm chat-transcript-empty">
                            "Start the conversation below."
                        </p>
                    }.into_any()
                } else {
                    let bubbles = list.into_iter().map(|turn| {
                        let question = turn.question.clone();
                        let answer = turn.response.as_ref().map(|r| r.answer.clone());
                        let model = turn.response.as_ref().map(|r| r.model.clone());
                        let hit_count = turn.response.as_ref().map(|r| r.hits.len()).unwrap_or(0);
                        let error = turn.error.clone();
                        view! {
                            <div class="chat-bubble chat-bubble-user">
                                <div class="chat-bubble-role">"You"</div>
                                <div class="chat-bubble-text">{question}</div>
                            </div>
                            <div class="chat-bubble chat-bubble-assistant">
                                <div class="chat-bubble-role">
                                    "Assistant"
                                    {model.map(|m| view! {
                                        <span class="chat-bubble-meta">" · "{m}" · "{hit_count}" hits"</span>
                                    })}
                                </div>
                                {if let Some(err) = error {
                                    view! { <div class="log-line-error">{err}</div> }.into_any()
                                } else if let Some(ans) = answer {
                                    view! { <div class="chat-bubble-text">{ans}</div> }.into_any()
                                } else {
                                    view! { <div class="chat-bubble-text muted">"…"</div> }.into_any()
                                }}
                            </div>
                        }
                    }).collect_view();
                    let pending = busy.get().then(|| view! {
                        <div class="chat-bubble chat-bubble-assistant">
                            <div class="chat-bubble-role">"Assistant"</div>
                            <div class="chat-bubble-text muted">"Thinking…"</div>
                        </div>
                    });
                    view! {
                        <>
                            {bubbles}
                            {pending}
                        </>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[component]
fn SourcesList(hits: Vec<QueryHit>) -> impl IntoView {
    view! {
        <div class="chat-sources">
            {hits.into_iter().enumerate().map(|(i, hit)| {
                let title = hit.document_title.clone()
                    .or_else(|| hit.source_ref_key.clone())
                    .unwrap_or_else(|| "Unknown source".to_string());
                let heading = hit.heading.clone();
                let score_display = format!("{:.3}", hit.score);
                let snippet = hit.snippet.clone();
                let source_link = match (hit.document_id, hit.char_start, hit.char_end) {
                    (Some(doc_id), Some(start), Some(end)) => Some(format!(
                        "/documents/by-id/{doc_id}?tab=source&ref_start={start}&ref_end={end}"
                    )),
                    (Some(doc_id), _, _) => Some(format!("/documents/by-id/{doc_id}?tab=source")),
                    _ => None,
                };
                view! {
                    <article class="chat-source">
                        <header class="chat-source-head">
                            <span class="playground-hit-rank">{i + 1}</span>
                            <span class="playground-hit-score">{score_display}</span>
                            <div class="chat-source-title">
                                <div class="playground-hit-doc">{title}</div>
                                {heading.map(|h| view! {
                                    <div class="playground-hit-heading muted">{h}</div>
                                })}
                            </div>
                        </header>
                        <p class="chat-source-snippet">{snippet}</p>
                        {source_link.map(|href| view! {
                            <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
                        })}
                    </article>
                }
            }).collect_view()}
        </div>
    }
}

fn build_metadata_filters(source_slug: &str, source_version: &str) -> Vec<MetadataFilterDto> {
    let mut out = Vec::new();
    let slug = source_slug.trim();
    if !slug.is_empty() {
        out.push(MetadataFilterDto {
            field: "source_slug".to_string(),
            value: slug.to_string(),
        });
    }
    let version = source_version.trim();
    if !version.is_empty() {
        out.push(MetadataFilterDto {
            field: "source_version".to_string(),
            value: version.to_string(),
        });
    }
    out
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
