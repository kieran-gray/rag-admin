use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::server::application::rerank::ports::{Reranker, RerankRequest, RerankScore};
use crate::server::application::rerank::RerankerRegistry;
use crate::server::application::AppError;
use crate::server::infrastructure::shared::clients::LlamaServerApi;
use crate::shared::reference_data::AiProviderKind;

const SYSTEM_PROMPT: &str = "You are a strict relevance grader. Decide if the given Document \
    answers, directly supports, or provides decisive evidence for the Query. Respond with a \
    single token: 'yes' if the document is relevant, 'no' otherwise. Do not output anything else.";

pub struct LlamaServerRerankClient {
    api: Arc<LlamaServerApi>,
}

impl LlamaServerRerankClient {
    pub fn new(api: Arc<LlamaServerApi>) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

pub fn register(registry: &mut RerankerRegistry, api: Arc<LlamaServerApi>) {
    registry.register(
        AiProviderKind::LlamaServer,
        LlamaServerRerankClient::new(api),
    );
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
    max_tokens: u32,
    logprobs: bool,
    top_logprobs: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatChoiceMessage>,
    #[serde(default)]
    logprobs: Option<ChoiceLogprobs>,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceLogprobs {
    #[serde(default)]
    content: Vec<TokenLogprob>,
}

#[derive(Deserialize)]
struct TokenLogprob {
    token: String,
    logprob: f64,
    #[serde(default)]
    top_logprobs: Vec<TopLogprob>,
}

#[derive(Deserialize)]
struct TopLogprob {
    token: String,
    logprob: f64,
}

#[async_trait]
impl Reranker for LlamaServerRerankClient {
    async fn rerank(&self, request: RerankRequest) -> Result<Vec<RerankScore>, AppError> {
        if request.documents.is_empty() {
            return Ok(Vec::new());
        }

        let base_url = self.api.base_url.trim().trim_end_matches('/');
        let url = format!("{base_url}/v1/chat/completions");
        let mut scores = Vec::with_capacity(request.documents.len());

        for (index, document) in request.documents.iter().enumerate() {
            let body = build_chat_request(&request.model, &request.query, document);
            let body_bytes = serde_json::to_vec(&body).map_err(|e| {
                AppError::Internal(format!("encode llama-server rerank body: {e}"))
            })?;
            let resp = self
                .api
                .request(
                    Method::POST,
                    &url,
                    body_bytes,
                    "application/json",
                    "llama-server rerank",
                )
                .await?;
            let score = parse_rerank_response(&resp)?;
            scores.push(RerankScore { index, score });
        }

        scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores)
    }
}

fn build_chat_request(model: &str, query: &str, document: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system",
                content: SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user",
                content: format!("Query: {query}\nDocument: {document}\nRelevant (yes/no):"),
            },
        ],
        temperature: 0.0,
        stream: false,
        max_tokens: 1,
        logprobs: true,
        top_logprobs: 5,
    }
}

fn parse_rerank_response(body: &str) -> Result<f32, AppError> {
    let parsed: ChatResponse = serde_json::from_str(body)
        .map_err(|e| AppError::Upstream(format!("parse llama-server rerank: {e}")))?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Upstream("llama-server rerank returned no choices".into()))?;

    if let Some(score) = score_from_logprobs(&choice.logprobs) {
        return Ok(score);
    }

    let content = choice
        .message
        .and_then(|m| m.content)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    Ok(text_fallback_score(&content))
}

fn score_from_logprobs(logprobs: &Option<ChoiceLogprobs>) -> Option<f32> {
    let entry = logprobs.as_ref()?.content.first()?;
    let mut yes_logit = f64::NEG_INFINITY;
    let mut no_logit = f64::NEG_INFINITY;

    consider_token(&entry.token, entry.logprob, &mut yes_logit, &mut no_logit);
    for alt in &entry.top_logprobs {
        consider_token(&alt.token, alt.logprob, &mut yes_logit, &mut no_logit);
    }

    if yes_logit.is_finite() || no_logit.is_finite() {
        Some(softmax_first(yes_logit, no_logit) as f32)
    } else {
        None
    }
}

fn consider_token(token: &str, logprob: f64, yes_logit: &mut f64, no_logit: &mut f64) {
    let normalised = token.trim().to_lowercase();
    if matches!(normalised.as_str(), "yes" | "y") {
        *yes_logit = yes_logit.max(logprob);
    } else if matches!(normalised.as_str(), "no" | "n") {
        *no_logit = no_logit.max(logprob);
    }
}

fn softmax_first(a: f64, b: f64) -> f64 {
    if !a.is_finite() && !b.is_finite() {
        return 0.0;
    }
    if !a.is_finite() {
        return 0.0;
    }
    if !b.is_finite() {
        return 1.0;
    }
    let m = a.max(b);
    let ea = (a - m).exp();
    let eb = (b - m).exp();
    ea / (ea + eb)
}

fn text_fallback_score(content: &str) -> f32 {
    let head = content
        .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .find(|s| !s.is_empty())
        .unwrap_or("");
    match head {
        "yes" | "y" => 1.0,
        "no" | "n" => 0.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logprobs_body(top: &[(&str, f64)]) -> String {
        let alts: String = top
            .iter()
            .map(|(t, lp)| format!(r#"{{"token":"{t}","logprob":{lp}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let first = top.first().copied().unwrap_or((" ", 0.0));
        format!(
            r#"{{
                "choices": [{{
                    "message": {{"content":"{tok}"}},
                    "logprobs": {{
                        "content": [{{
                            "token": "{tok}",
                            "logprob": {lp},
                            "top_logprobs": [{alts}]
                        }}]
                    }}
                }}]
            }}"#,
            tok = first.0,
            lp = first.1,
            alts = alts
        )
    }

    #[test]
    fn scores_close_to_one_when_yes_logit_dominates() {
        let body = logprobs_body(&[(" yes", -0.1), (" no", -3.0)]);
        let score = parse_rerank_response(&body).unwrap();
        assert!(score > 0.9, "score was {score}");
    }

    #[test]
    fn scores_close_to_zero_when_no_logit_dominates() {
        let body = logprobs_body(&[(" no", -0.1), (" yes", -3.0)]);
        let score = parse_rerank_response(&body).unwrap();
        assert!(score < 0.1, "score was {score}");
    }

    #[test]
    fn falls_back_to_text_when_logprobs_missing() {
        let body = r#"{ "choices": [{ "message": { "content": "yes" } }] }"#;
        let score = parse_rerank_response(body).unwrap();
        assert_eq!(score, 1.0);
    }

    #[test]
    fn text_fallback_returns_zero_for_unknown_content() {
        let body = r#"{ "choices": [{ "message": { "content": "maybe" } }] }"#;
        let score = parse_rerank_response(body).unwrap();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn handles_uppercase_yes_in_logprobs() {
        let body = logprobs_body(&[("Yes", -0.05), ("No", -4.0)]);
        let score = parse_rerank_response(&body).unwrap();
        assert!(score > 0.95);
    }
}
