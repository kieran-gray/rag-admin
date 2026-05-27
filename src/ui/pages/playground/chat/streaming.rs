use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{ChatRequest, ChatStreamMeta, QueryHit, Timings};

#[cfg(feature = "hydrate")]
use crate::ui::components::playground::sse_client::{
    start_chat_stream, ChatStreamCallbacks, ChatStreamEvent, ChatStreamHandle,
};

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub(super) enum TurnStatus {
    Pending,
    Streaming,
    Done,
    Aborted,
    Failed,
}

#[derive(Clone)]
pub(super) struct ChatTurn {
    pub turn_id: Uuid,
    pub question: String,
    pub status: TurnStatus,
    pub meta: Option<ChatStreamMeta>,
    pub answer: String,
    pub timings: Option<Timings>,
    pub error: Option<String>,
    pub request_json: String,
}

impl ChatTurn {
    pub(super) fn hits(&self) -> &[QueryHit] {
        self.meta.as_ref().map(|m| m.hits.as_slice()).unwrap_or(&[])
    }
}

#[cfg(feature = "hydrate")]
pub(super) type StreamSlot = Option<ChatStreamHandle>;
#[cfg(not(feature = "hydrate"))]
pub(super) type StreamSlot = Option<()>;

#[cfg(feature = "hydrate")]
pub(super) fn run_stream(
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
pub(super) fn run_stream(
    _req: &ChatRequest,
    _turn_id: Uuid,
    _turns: RwSignal<Vec<ChatTurn>>,
    busy: RwSignal<bool>,
    _active_stream: StoredValue<StreamSlot>,
) {
    busy.set(false);
}

#[cfg(feature = "hydrate")]
pub(super) fn cancel_active_stream(active_stream: StoredValue<StreamSlot>) {
    active_stream.update_value(|slot| {
        if let Some(handle) = slot.take() {
            handle.cancel();
        }
    });
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn cancel_active_stream(_active_stream: StoredValue<StreamSlot>) {}

#[cfg(feature = "hydrate")]
pub(super) fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        drop(clipboard.write_text(text));
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn copy_to_clipboard(_text: &str) {}
