use leptos::prelude::*;

use crate::shared::contracts::{ChatRequest, ChatResponse};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::chat::ChatServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = ChatQuery, prefix = "/api", endpoint = "chat_query")]
pub async fn chat_query(req: ChatRequest) -> Result<ChatResponse, ServerFnError> {
    ctx::<Arc<ChatServices>>()?
        .chat_service
        .chat(req)
        .await
        .map_err(|e| map_app_error(&e))
}
