use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use uuid::Uuid;

use crate::server_functions::configuration::get_retrieval_profiles;
use crate::shared::contracts::{
    ChatRequest, ChatStreamMeta, MetadataFilterDto, QueryHit, RetrievalProfileDto, Timings,
};
use crate::ui::components::playground::metadata_filters::metadata_filters_from_signal;
use crate::ui::components::playground::{
    LatencyBadge, QueryInput, RequestInspector, RetrievalControls,
};
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

#[cfg(feature = "hydrate")]
use crate::ui::components::playground::sse_client::{
    start_chat_stream, ChatStreamCallbacks, ChatStreamEvent, ChatStreamHandle,
};

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
enum TurnStatus {
    Pending,
    Streaming,
    Done,
    Aborted,
    Failed,
}

#[derive(Clone)]
struct ChatTurn {
    turn_id: Uuid,
    question: String,
    status: TurnStatus,
    meta: Option<ChatStreamMeta>,
    answer: String,
    timings: Option<Timings>,
    error: Option<String>,
    request_json: String,
}

impl ChatTurn {
    fn hits(&self) -> &[QueryHit] {
        self.meta.as_ref().map(|m| m.hits.as_slice()).unwrap_or(&[])
    }
}

#[cfg(feature = "hydrate")]
type StreamSlot = Option<ChatStreamHandle>;
#[cfg(not(feature = "hydrate"))]
type StreamSlot = Option<()>;

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

    let question = RwSignal::new(String::new());
    let (retrieval_profile_id, set_retrieval_profile_id) = signal(initial_retrieval_profile);
    let top_k = RwSignal::new(8u32);
    let min_score = RwSignal::new(0.4f32);
    let filters = RwSignal::new(Vec::<MetadataFilterDto>::new());
    let busy = RwSignal::new(false);
    let turns = RwSignal::new(Vec::<ChatTurn>::new());
    let selected_turn = RwSignal::new(None::<Uuid>);
    let active_stream: StoredValue<StreamSlot> = StoredValue::new(None);

    let submit_question = move |question_text: String| {
        if busy.get_untracked() || question_text.trim().is_empty() {
            return;
        }
        cancel_active_stream(active_stream);

        let pid = retrieval_profile_id.get_untracked();
        let req = ChatRequest {
            retrieval_profile_id: pid,
            query: question_text.clone(),
            top_k: top_k.get_untracked(),
            min_score: min_score.get_untracked(),
            metadata_filters: metadata_filters_from_signal(filters),
        };
        let request_json = serde_json::to_string_pretty(&req).unwrap_or_default();
        let turn_id = Uuid::new_v4();

        busy.set(true);
        question.set(String::new());
        selected_turn.set(Some(turn_id));
        turns.update(|t| {
            t.push(ChatTurn {
                turn_id,
                question: question_text,
                status: TurnStatus::Pending,
                meta: None,
                answer: String::new(),
                timings: None,
                error: None,
                request_json,
            });
        });

        run_stream(&req, turn_id, turns, busy, active_stream);
    };

    let submit = Callback::new(move |()| {
        submit_question(question.get_untracked());
    });

    let stop = Callback::new(move |()| {
        cancel_active_stream(active_stream);
        turns.update(|t| {
            if let Some(last) = t.last_mut() {
                if matches!(last.status, TurnStatus::Pending | TurnStatus::Streaming) {
                    last.status = TurnStatus::Aborted;
                }
            }
        });
        busy.set(false);
    });

    let clear_history = move |_| {
        cancel_active_stream(active_stream);
        turns.set(Vec::new());
        selected_turn.set(None);
        busy.set(false);
    };

    let edit_turn = move |turn_id: Uuid| {
        cancel_active_stream(active_stream);
        let q = turns.with(|t| {
            t.iter()
                .find(|x| x.turn_id == turn_id)
                .map(|x| x.question.clone())
        });
        if let Some(q) = q {
            question.set(q);
            turns.update(|t| {
                if let Some(idx) = t.iter().position(|x| x.turn_id == turn_id) {
                    t.truncate(idx);
                }
            });
            selected_turn.set(None);
            busy.set(false);
        }
    };

    let regenerate_turn = move |turn_id: Uuid| {
        let q = turns.with(|t| {
            t.iter()
                .find(|x| x.turn_id == turn_id)
                .map(|x| x.question.clone())
        });
        if let Some(q) = q {
            turns.update(|t| {
                if let Some(idx) = t.iter().position(|x| x.turn_id == turn_id) {
                    t.remove(idx);
                }
            });
            submit_question(q);
        }
    };

    let select_turn = move |turn_id: Uuid| {
        selected_turn.set(Some(turn_id));
    };

    let last_request_body = Signal::derive(move || {
        turns.with(|t| t.last().map(|t| t.request_json.clone()).unwrap_or_default())
    });

    let focused_hits = Signal::derive(move || {
        turns.with(|t| {
            let selected = selected_turn.get();
            let turn = selected
                .and_then(|id| t.iter().find(|x| x.turn_id == id))
                .or_else(|| t.last());
            turn.map(|x| (x.turn_id, x.hits().to_vec()))
        })
    });

    view! {
        <div class="playground-grid">
            <div class="playground-main">
                <Surface title="Conversation".to_string() actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <RetrievalProfilePicker
                            profiles=retrieval_profiles_stored.get_value()
                            value=retrieval_profile_id
                            set_value=set_retrieval_profile_id
                        />
                        <button
                            type="button"
                            class="btn btn-sm btn-ghost"
                            disabled=move || busy.get() || turns.with(Vec::is_empty)
                            on:click=clear_history
                        >
                            "Clear"
                        </button>
                    </div>
                }.into_any())>
                    <div class="playground-body">
                        <ChatTranscript
                            turns=turns
                            selected_turn=selected_turn
                            on_select=select_turn
                            on_edit=edit_turn
                            on_regenerate=regenerate_turn
                        />

                        <QueryInput
                            value=question
                            busy=Signal::from(busy)
                            on_submit=submit
                            on_stop=stop
                            placeholder="Ask a question…".to_string()
                            submit_label="Send".to_string()
                            busy_label="Asking…".to_string()
                        />

                        <RetrievalControls top_k=top_k min_score=min_score filters=filters />

                        {move || {
                            let has_request = turns.with(|t| !t.is_empty());
                            has_request.then(|| view! {
                                <RequestInspector body=last_request_body label="Inspect last request".to_string() />
                            })
                        }}
                    </div>
                </Surface>
            </div>

            <aside class="playground-sidebar">
                <Surface title="Sources".to_string()>
                    {move || {
                        match focused_hits.get() {
                            None => view! {
                                <p class="muted text-sm">"Ask a question to see the retrieved chunks here."</p>
                            }.into_any(),
                            Some((_, hits)) if hits.is_empty() => view! {
                                <p class="muted text-sm">"No chunks retrieved for the selected turn."</p>
                            }.into_any(),
                            Some((_, hits)) => view! { <SourcesList hits=hits /> }.into_any(),
                        }
                    }}
                </Surface>
            </aside>
        </div>
    }
}

#[cfg(feature = "hydrate")]
fn run_stream(
    req: &ChatRequest,
    turn_id: Uuid,
    turns: RwSignal<Vec<ChatTurn>>,
    busy: RwSignal<bool>,
    active_stream: StoredValue<StreamSlot>,
) {
    let body_json = serde_json::to_string(req).unwrap_or_else(|_| "{}".to_string());

    let on_event = Box::new(move |event: ChatStreamEvent| {
        turns.update(|t| {
            let Some(turn) = t.iter_mut().find(|x| x.turn_id == turn_id) else {
                return;
            };
            match event {
                ChatStreamEvent::Meta(meta) => {
                    turn.meta = Some(meta);
                    turn.status = TurnStatus::Streaming;
                }
                ChatStreamEvent::Delta(delta) => {
                    turn.status = TurnStatus::Streaming;
                    turn.answer.push_str(&delta.text);
                }
                ChatStreamEvent::Done(done) => {
                    turn.timings = Some(done.timings);
                    turn.status = TurnStatus::Done;
                }
                ChatStreamEvent::Error(err) => {
                    turn.error = Some(err.message);
                    turn.status = TurnStatus::Failed;
                }
            }
        });
    });

    let on_done = Box::new(move || {
        turns.update(|t| {
            if let Some(turn) = t.iter_mut().find(|x| x.turn_id == turn_id) {
                if matches!(turn.status, TurnStatus::Pending | TurnStatus::Streaming) {
                    turn.status = TurnStatus::Done;
                }
            }
        });
        busy.set(false);
        active_stream.set_value(None);
    });

    let on_fail = Box::new(move |message: String| {
        turns.update(|t| {
            if let Some(turn) = t.iter_mut().find(|x| x.turn_id == turn_id) {
                if turn.error.is_none() {
                    turn.error = Some(message);
                }
                turn.status = TurnStatus::Failed;
            }
        });
        busy.set(false);
        active_stream.set_value(None);
    });

    match start_chat_stream(
        "/api/chat_stream",
        &body_json,
        ChatStreamCallbacks {
            on_event,
            on_done,
            on_fail,
        },
    ) {
        Ok(handle) => {
            active_stream.set_value(Some(handle));
        }
        Err(message) => {
            turns.update(|t| {
                if let Some(turn) = t.iter_mut().find(|x| x.turn_id == turn_id) {
                    turn.error = Some(message);
                    turn.status = TurnStatus::Failed;
                }
            });
            busy.set(false);
        }
    }
}

#[cfg(not(feature = "hydrate"))]
fn run_stream(
    _req: &ChatRequest,
    _turn_id: Uuid,
    _turns: RwSignal<Vec<ChatTurn>>,
    busy: RwSignal<bool>,
    _active_stream: StoredValue<StreamSlot>,
) {
    busy.set(false);
}

#[cfg(feature = "hydrate")]
fn cancel_active_stream(active_stream: StoredValue<StreamSlot>) {
    active_stream.update_value(|slot| {
        if let Some(handle) = slot.take() {
            handle.cancel();
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn cancel_active_stream(_active_stream: StoredValue<StreamSlot>) {}

#[cfg(feature = "hydrate")]
fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        drop(clipboard.write_text(text));
    }
}

#[cfg(not(feature = "hydrate"))]
fn copy_to_clipboard(_text: &str) {}

#[component]
fn ChatTranscript(
    turns: RwSignal<Vec<ChatTurn>>,
    selected_turn: RwSignal<Option<Uuid>>,
    on_select: impl Fn(Uuid) + Copy + Send + Sync + 'static,
    on_edit: impl Fn(Uuid) + Copy + Send + Sync + 'static,
    on_regenerate: impl Fn(Uuid) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="chat-transcript">
            {move || {
                let list = turns.get();
                if list.is_empty() {
                    view! {
                        <p class="muted text-sm chat-transcript-empty">
                            "Start the conversation below."
                        </p>
                    }.into_any()
                } else {
                    let last_id = list.last().map(|t| t.turn_id);
                    list.into_iter().map(|turn| {
                        let is_selected = Memo::new(move |_| {
                            match selected_turn.get() {
                                Some(id) => id == turn.turn_id,
                                None => last_id == Some(turn.turn_id),
                            }
                        });
                        view! {
                            <ChatTurnView
                                turn=turn
                                is_selected=Signal::derive(move || is_selected.get())
                                on_select=on_select
                                on_edit=on_edit
                                on_regenerate=on_regenerate
                            />
                        }
                    }).collect_view().into_any()
                }
            }}
        </div>
    }
}

#[component]
fn ChatTurnView(
    turn: ChatTurn,
    is_selected: Signal<bool>,
    on_select: impl Fn(Uuid) + Copy + Send + Sync + 'static,
    on_edit: impl Fn(Uuid) + Copy + Send + Sync + 'static,
    on_regenerate: impl Fn(Uuid) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let turn_id = turn.turn_id;
    let question = turn.question.clone();
    let status = turn.status.clone();
    let meta = turn.meta.clone();
    let answer = turn.answer.clone();
    let timings = turn.timings;
    let error = turn.error.clone();

    let (prompt_open, set_prompt_open) = signal(false);

    let model = meta.as_ref().map(|m| m.model.clone());
    let hit_count = meta.as_ref().map(|m| m.hits.len()).unwrap_or(0);
    let prompt_text = meta.as_ref().map(|m| m.prompt.clone());

    let is_streaming = matches!(status, TurnStatus::Pending | TurnStatus::Streaming);
    let is_aborted = matches!(status, TurnStatus::Aborted);
    let is_failed = matches!(status, TurnStatus::Failed);

    let answer_for_copy = answer.clone();
    let copy_label = RwSignal::new("Copy");
    let copy_answer = move |_| {
        copy_to_clipboard(&answer_for_copy);
        copy_label.set("Copied");
        leptos::task::spawn_local(async move {
            TimeoutFuture::new(1500).await;
            copy_label.set("Copy");
        });
    };

    let bubble_class = move || {
        if is_selected.get() {
            "chat-bubble chat-bubble-assistant chat-bubble-selected"
        } else {
            "chat-bubble chat-bubble-assistant"
        }
    };

    view! {
        <div class="chat-bubble chat-bubble-user" on:click=move |_| on_select(turn_id)>
            <div class="chat-bubble-head">
                <div class="chat-bubble-role">"You"</div>
                <button
                    type="button"
                    class="chat-bubble-icon-btn"
                    title="Edit and resend"
                    on:click=move |ev| { ev.stop_propagation(); on_edit(turn_id); }
                >
                    "✎"
                </button>
            </div>
            <div class="chat-bubble-text">{question}</div>
        </div>
        <div class=bubble_class on:click=move |_| on_select(turn_id)>
            <div class="chat-bubble-head">
                <div class="chat-bubble-role">
                    "Assistant"
                    {model.clone().map(|m| view! {
                        <span class="chat-bubble-meta">" · "{m}" · "{hit_count}" hits"</span>
                    })}
                    {timings.map(|t| view! {
                        <span class="chat-bubble-meta"> " · " <LatencyBadge timings=t /> </span>
                    })}
                    {is_aborted.then(|| view! { <span class="chat-bubble-meta"> " · stopped"</span> })}
                </div>
            </div>

            {if is_failed {
                error.clone().map(|e| view! {
                    <div class="log-line-error">{e}</div>
                }).into_any()
            } else if answer.is_empty() && is_streaming {
                view! { <div class="chat-bubble-text muted chat-bubble-pending">"Thinking…"</div> }.into_any()
            } else {
                let with_cursor = is_streaming;
                view! {
                    <div class="chat-bubble-text">
                        {answer.clone()}
                        {with_cursor.then(|| view! { <span class="chat-bubble-cursor" aria-hidden="true"></span> })}
                    </div>
                }.into_any()
            }}

            <div class="chat-bubble-actions">
                {(!is_streaming).then(|| view! {
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        on:click=move |ev| { ev.stop_propagation(); on_regenerate(turn_id); }
                    >
                        "Regenerate"
                    </button>
                })}
                {(!answer.is_empty()).then(|| view! {
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        on:click=move |ev| { ev.stop_propagation(); copy_answer(ev); }
                    >
                        {move || copy_label.get()}
                    </button>
                })}
                {prompt_text.clone().map(|_| view! {
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        on:click=move |ev| {
                            ev.stop_propagation();
                            set_prompt_open.update(|v| *v = !*v);
                        }
                    >
                        {move || if prompt_open.get() { "Hide prompt" } else { "Show prompt" }}
                    </button>
                })}
            </div>

            {move || (prompt_open.get() && prompt_text.is_some()).then(|| view! {
                <pre class="chat-bubble-prompt">{prompt_text.clone().unwrap_or_default()}</pre>
            })}
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
