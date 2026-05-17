use leptos::prelude::*;

use crate::contracts::{ChatRequest, ChatResponse};

#[cfg(feature = "ssr")]
use crate::server::application::chat::ChatService;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = ChatQuery, prefix = "/api", endpoint = "chat_query")]
pub async fn chat_query(req: ChatRequest) -> Result<ChatResponse, ServerFnError> {
    ctx::<Arc<ChatService>>()?
        .chat(req)
        .await
        .map_err(|e| map_app_error(&e))
}
