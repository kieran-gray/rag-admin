use bytes::Bytes;

use crate::shared::contracts::{ChatStreamDelta, ChatStreamDone, ChatStreamError, ChatStreamMeta};

#[derive(Debug, Clone)]
pub enum ChatSseEvent {
    Meta(ChatStreamMeta),
    Delta(ChatStreamDelta),
    Done(ChatStreamDone),
    Error(ChatStreamError),
}

impl ChatSseEvent {
    pub fn encode(&self) -> Bytes {
        match self {
            Self::Meta(payload) => encode_event("meta", payload),
            Self::Delta(payload) => encode_event("delta", payload),
            Self::Done(payload) => encode_event("done", payload),
            Self::Error(payload) => encode_event("error", payload),
        }
    }
}

fn encode_event<T: serde::Serialize>(name: &'static str, payload: &T) -> Bytes {
    let json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    Bytes::from(format!("event: {name}\ndata: {json}\n\n"))
}
