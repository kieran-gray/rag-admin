use leptos::prelude::*;

use crate::shared::{EmbedInputType, EmbedManyResult, EmbedMatrixResult, EmbedResult};

#[cfg(feature = "ssr")]
use crate::server::setup::compose::catalog::CatalogServices;
#[cfg(feature = "ssr")]
use crate::server::setup::compose::platform::PlatformServices;
#[cfg(feature = "ssr")]
use crate::server_functions::error::{ctx, map_app_error};
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
async fn resolve_embedding_model_id(model: &str) -> Result<Uuid, ServerFnError> {
    let configuration = ctx::<Arc<CatalogServices>>()?
        .configuration_query_service
        .get()
        .await
        .map_err(|e| map_app_error(&e))?;
    configuration
        .embedding_models
        .iter()
        .find(|m| m.model == model)
        .map(|m| m.embedding_model_id)
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "embedding model '{model}' is not registered in the configuration"
            ))
        })
}

#[server(name = EmbedTexts, prefix = "/api", endpoint = "embed_texts")]
pub async fn embed_texts(
    model: String,
    text_a: String,
    text_b: String,
    type_a: EmbedInputType,
    type_b: EmbedInputType,
) -> Result<EmbedResult, ServerFnError> {
    let embedding_model_id = resolve_embedding_model_id(&model).await?;
    ctx::<Arc<PlatformServices>>()?
        .embedding_service
        .embed_texts(embedding_model_id, &text_a, &text_b, type_a, type_b)
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = EmbedMany, prefix = "/api", endpoint = "embed_many")]
pub async fn embed_many(
    model: String,
    query: String,
    candidates: Vec<String>,
    query_type: EmbedInputType,
    candidate_type: EmbedInputType,
) -> Result<EmbedManyResult, ServerFnError> {
    let embedding_model_id = resolve_embedding_model_id(&model).await?;
    ctx::<Arc<PlatformServices>>()?
        .embedding_service
        .embed_many(
            embedding_model_id,
            &query,
            &candidates,
            query_type,
            candidate_type,
        )
        .await
        .map_err(|e| map_app_error(&e))
}

#[server(name = EmbedMatrix, prefix = "/api", endpoint = "embed_matrix")]
pub async fn embed_matrix(
    model: String,
    texts: Vec<String>,
    input_type: EmbedInputType,
) -> Result<EmbedMatrixResult, ServerFnError> {
    let embedding_model_id = resolve_embedding_model_id(&model).await?;
    ctx::<Arc<PlatformServices>>()?
        .embedding_service
        .embed_matrix(embedding_model_id, &texts, input_type)
        .await
        .map_err(|e| map_app_error(&e))
}
