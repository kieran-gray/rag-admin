use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Query};
use axum::response::Response;
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::server::event_sourcing::event_bus::EventBus;

#[derive(Debug, Deserialize)]
pub struct EventsWsQuery {
    pub stream_id: Option<Uuid>,
}

pub async fn events_ws_handler(
    Extension(event_bus): Extension<Arc<EventBus>>,
    Query(query): Query<EventsWsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, event_bus, query.stream_id))
}

async fn handle_socket(mut socket: WebSocket, event_bus: Arc<EventBus>, stream_id: Option<Uuid>) {
    let mut subscription = event_bus.subscribe();

    loop {
        let event = match subscription.recv().await {
            Ok(event) => event,
            Err(RecvError::Lagged(n)) => {
                warn!(?stream_id, dropped = n, "events ws subscriber lagged");
                continue;
            }
            Err(RecvError::Closed) => {
                debug!(?stream_id, "events bus closed");
                return;
            }
        };

        if let Some(filter) = stream_id {
            if event.stream_id != filter {
                continue;
            }
        }

        let payload = match serde_json::to_string(event.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "failed to serialize PublishedEvent for ws");
                continue;
            }
        };

        if socket.send(Message::Text(payload.into())).await.is_err() {
            return;
        }
    }
}
