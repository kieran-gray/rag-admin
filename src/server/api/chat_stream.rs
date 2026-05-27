use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use axum::extract::Extension;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures_util::stream::{Stream, StreamExt};

use crate::server::application::chat::{ChatService, ChatSseEvent};
use crate::server::setup::compose::chat::ChatServices;
use crate::shared::contracts::{ChatRequest, ChatStreamError};

pub async fn chat_stream_handler(
    Extension(chat): Extension<Arc<ChatServices>>,
    Json(request): Json<ChatRequest>,
) -> (
    HeaderMap,
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
) {
    let mut headers = HeaderMap::new();
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    headers.insert(
        "cache-control",
        HeaderValue::from_static("no-cache, no-transform"),
    );

    let stream = build_stream(chat.chat_service.clone(), request);

    (
        headers,
        Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    )
}

fn build_stream(
    chat_service: Arc<ChatService>,
    request: ChatRequest,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream! {
        let stream_result = chat_service.chat_stream(request).await;
        match stream_result {
            Ok(mut events) => {
                while let Some(event) = events.next().await {
                    yield Ok(to_axum_event(event));
                }
            }
            Err(e) => {
                yield Ok(to_axum_event(ChatSseEvent::Error(ChatStreamError {
                    message: e.to_string(),
                })));
            }
        }
    }
}

fn to_axum_event(event: ChatSseEvent) -> Event {
    match event {
        ChatSseEvent::Meta(payload) => Event::default()
            .event("meta")
            .json_data(&payload)
            .unwrap_or_else(|e| error_event(&e)),
        ChatSseEvent::Delta(payload) => Event::default()
            .event("delta")
            .json_data(&payload)
            .unwrap_or_else(|e| error_event(&e)),
        ChatSseEvent::Done(payload) => Event::default()
            .event("done")
            .json_data(&payload)
            .unwrap_or_else(|e| error_event(&e)),
        ChatSseEvent::Error(payload) => Event::default()
            .event("error")
            .json_data(&payload)
            .unwrap_or_else(|e| error_event(&e)),
    }
}

fn error_event(err: &axum::Error) -> Event {
    Event::default()
        .event("error")
        .data(format!("{{\"message\":\"{err}\"}}"))
}
