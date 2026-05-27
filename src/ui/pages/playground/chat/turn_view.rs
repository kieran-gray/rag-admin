use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use uuid::Uuid;

use crate::ui::components::playground::LatencyBadge;

use super::streaming::{copy_to_clipboard, ChatTurn, TurnStatus};

#[component]
pub(super) fn ChatTranscript(
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

#[allow(clippy::needless_pass_by_value)]
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
