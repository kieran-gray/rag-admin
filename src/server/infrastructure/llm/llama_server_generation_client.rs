use std::sync::Arc;

use async_stream::stream;
use async_trait::async_trait;
use futures_util::stream::StreamExt;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::llm::ports::{
    GenerationClient, GenerationRequest, GenerationResponse, GenerationResponseFormat,
    GenerationTokenStream,
};
use crate::server::application::llm::GenerationClientRegistry;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::LlamaServerApi;
use crate::server::infrastructure::shared::sse::sse_data_stream;
use crate::shared::reference_data::AiProviderKind;

pub struct LlamaServerGenerationClient {
    api: Arc<LlamaServerApi>,
}

impl LlamaServerGenerationClient {
    pub fn new(api: Arc<LlamaServerApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

pub fn register(registry: &mut GenerationClientRegistry, api: Arc<LlamaServerApi>) {
    registry.register(
        AiProviderKind::LlamaServer,
        LlamaServerGenerationClient::new(api),
    );
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

fn build_chat_request(request: GenerationRequest, stream: bool) -> ChatRequest {
    ChatRequest {
        model: request.model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: request.system,
            },
            ChatMessage {
                role: "user",
                content: request.user,
            },
        ],
        temperature: request.temperature,
        stream,
        response_format: match request.response_format {
            GenerationResponseFormat::Text => None,
            GenerationResponseFormat::Json => Some(ResponseFormat {
                kind: "json_object",
            }),
        },
    }
}

#[derive(Deserialize)]
struct ChunkResponse {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    #[serde(default)]
    delta: ChunkDelta,
}

#[derive(Deserialize, Default)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}

pub(super) enum ChunkOutcome {
    Token(String),
    Skip,
    Done,
}

fn parse_chunk(data: &str) -> Result<ChunkOutcome, AppError> {
    if data.trim() == "[DONE]" {
        return Ok(ChunkOutcome::Done);
    }
    let chunk: ChunkResponse = serde_json::from_str(data)
        .map_err(|e| AppError::Upstream(format!("parse llama-server stream chunk: {e}")))?;
    let content = chunk
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content);
    Ok(match content {
        Some(t) if !t.is_empty() => ChunkOutcome::Token(t),
        _ => ChunkOutcome::Skip,
    })
}

fn parse_chat_response(body: &str) -> Result<GenerationResponse, AppError> {
    let parsed: ChatResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse llama-server generate: {e}")))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or_else(|| AppError::Upstream("llama-server generate returned no choices".into()))?;
    Ok(GenerationResponse { content })
}

#[async_trait]
impl GenerationClient for LlamaServerGenerationClient {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AppError> {
        let base_url = self.api.base_url.trim().trim_end_matches('/');
        let url = format!("{base_url}/v1/chat/completions");

        let body = build_chat_request(request, false);
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode llama-server generate body: {e}")))?;

        let resp = self
            .api
            .request(
                Method::POST,
                &url,
                body_bytes,
                "application/json",
                "llama-server generate",
            )
            .await?;

        parse_chat_response(&resp)
    }

    async fn generate_stream(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationTokenStream, AppError> {
        let base_url = self.api.base_url.trim().trim_end_matches('/');
        let url = format!("{base_url}/v1/chat/completions");

        let body = build_chat_request(request, true);
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            AppError::Internal(format!("encode llama-server generate stream body: {e}"))
        })?;

        let byte_stream = self
            .api
            .request_stream(Method::POST, &url, body_bytes, "application/json")
            .await?;

        let mut data_stream = sse_data_stream(byte_stream);
        let s = stream! {
            while let Some(item) = data_stream.next().await {
                match item {
                    Ok(data) => match parse_chunk(&data) {
                        Ok(ChunkOutcome::Token(t)) => yield Ok(t),
                        Ok(ChunkOutcome::Skip) => {}
                        Ok(ChunkOutcome::Done) => return,
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    },
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }
        };
        Ok(s.boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sample_request() -> GenerationRequest {
        GenerationRequest {
            model: "gemma3-12b".into(),
            system: "you are a helpful assistant".into(),
            user: "hello".into(),
            temperature: 0.2,
            response_format: GenerationResponseFormat::Text,
        }
    }

    #[test]
    fn builds_openai_chat_request_with_system_and_user_messages() {
        let body = build_chat_request(sample_request(), false);
        let json: Value = serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap();

        assert_eq!(json["model"], "gemma3-12b");
        assert_eq!(json["stream"], false);
        assert_eq!(json["temperature"].as_f64().unwrap(), 0.2);
        assert!(json.get("response_format").is_none());
        let messages = json["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "you are a helpful assistant");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hello");
    }

    #[test]
    fn json_response_format_serialises_as_json_object() {
        let req = GenerationRequest {
            response_format: GenerationResponseFormat::Json,
            ..sample_request()
        };
        let body = build_chat_request(req, false);
        let json: Value = serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap();
        assert_eq!(json["response_format"]["type"], "json_object");
    }

    #[test]
    fn parses_chat_completion_response() {
        let body = r#"{
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "hello there" },
                "finish_reason": "stop"
            }]
        }"#;
        let resp = parse_chat_response(body).unwrap();
        assert_eq!(resp.content, "hello there");
    }

    #[test]
    fn returns_error_on_empty_choices() {
        let body = r#"{ "choices": [] }"#;
        let err = parse_chat_response(body).unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }

    #[test]
    fn returns_error_on_missing_content() {
        let body = r#"{
            "choices": [{
                "message": { "role": "assistant" }
            }]
        }"#;
        let err = parse_chat_response(body).unwrap_err();
        assert!(matches!(err, AppError::Upstream(_)));
    }
}
