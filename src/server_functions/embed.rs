use leptos::prelude::*;

use crate::shared::EmbedResult;

#[cfg(feature = "ssr")]
use crate::server::application::configuration::ConfigurationQueryService;
#[cfg(feature = "ssr")]
use crate::server::application::embedding::EmbeddingService;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[server(name = EmbedTexts, prefix = "/api", endpoint = "embed_texts")]
pub async fn embed_texts(
    model: String,
    text_a: String,
    text_b: String,
) -> Result<EmbedResult, ServerFnError> {
    let configuration = ctx::<Arc<ConfigurationQueryService>>()?
        .get()
        .await
        .map_err(map_app_error)?;
    let embedding_model_id = configuration
        .embedding_models
        .iter()
        .find(|m| m.model == model)
        .map(|m| m.embedding_model_id)
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "embedding model '{model}' is not registered in the configuration"
            ))
        })?;

    ctx::<Arc<EmbeddingService>>()?
        .embed_texts(embedding_model_id, &text_a, &text_b)
        .await
        .map_err(map_app_error)
}
