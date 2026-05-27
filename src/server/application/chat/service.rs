use std::slice;
use std::sync::Arc;
use std::time::Instant;

use async_stream::stream;
use futures_util::stream::{BoxStream, StreamExt};
use uuid::Uuid;

use crate::server::application::chat::ChatSseEvent;
use crate::server::application::configuration::RetrievalProfileResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::ports::vector_index::{
    MetadataFilter, MetadataFilterOperation, VectorQuery,
};
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::llm::ports::GenerationResponseFormat;
use crate::server::application::llm::service::GenerationPrompt;
use crate::server::application::llm::GenerationService;
use crate::server::application::rerank::{rerank_fetch_k, RerankService};
use crate::server::application::AppError;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::shared::contracts::{
    ChatRequest, ChatResponse, ChatStreamDelta, ChatStreamDone, ChatStreamError, ChatStreamMeta,
    QueryHit, Timings,
};

const SYSTEM_PROMPT: &str = include_str!("prompts/chat_system_prompt.txt");
const SNIPPET_MAX_CHARS: usize = 320;
const GENERATION_TEMPERATURE: f32 = 0.2;
const NO_CONTEXT_ANSWER: &str = "I don't see that in the indexed documents.";

struct RetrievedChunk {
    hit: QueryHit,
    text: String,
}

struct PreparedChat {
    retrieval_profile_id: Uuid,
    generation_model_id: Uuid,
    generation_model_name: String,
    query: String,
    retrieved: Vec<RetrievedChunk>,
    system_prompt: String,
    user_prompt: String,
    timings: Timings,
}

pub struct ChatService {
    retrieval_profile_resolver: Arc<RetrievalProfileResolver>,
    embedding_service: Arc<EmbeddingService>,
    vector_index_resolver: Arc<VectorIndexResolver>,
    generation_service: Arc<GenerationService>,
    rerank_service: Arc<RerankService>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
}

impl ChatService {
    pub fn new(
        retrieval_profile_resolver: Arc<RetrievalProfileResolver>,
        embedding_service: Arc<EmbeddingService>,
        vector_index_resolver: Arc<VectorIndexResolver>,
        generation_service: Arc<GenerationService>,
        rerank_service: Arc<RerankService>,
        source_document_repository: Arc<dyn SourceDocumentRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            retrieval_profile_resolver,
            embedding_service,
            vector_index_resolver,
            generation_service,
            rerank_service,
            source_document_repository,
        })
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        let started = Instant::now();
        let mut prepared = self.prepare(req).await?;

        let answer = if prepared.retrieved.is_empty() {
            NO_CONTEXT_ANSWER.to_string()
        } else {
            let generate_start = Instant::now();
            let response = self
                .generation_service
                .generate(
                    prepared.generation_model_id,
                    GenerationPrompt {
                        system: prepared.system_prompt.clone(),
                        user: prepared.user_prompt.clone(),
                        temperature: GENERATION_TEMPERATURE,
                        response_format: GenerationResponseFormat::Text,
                    },
                )
                .await?;
            prepared.timings.generate_ms = Some(elapsed_ms(generate_start));
            response.content
        };

        prepared.timings.total_ms = elapsed_ms(started);

        Ok(ChatResponse {
            retrieval_profile_id: prepared.retrieval_profile_id,
            query: prepared.query,
            answer,
            model: prepared.generation_model_name,
            hits: prepared.retrieved.into_iter().map(|r| r.hit).collect(),
            timings: prepared.timings,
        })
    }

    pub async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<BoxStream<'static, ChatSseEvent>, AppError> {
        let started = Instant::now();
        let prepared = self.prepare(req).await?;
        let generation_service = self.generation_service.clone();

        let s = stream! {
            let hits: Vec<QueryHit> = prepared
                .retrieved
                .iter()
                .map(|r| r.hit.clone())
                .collect();

            yield ChatSseEvent::Meta(ChatStreamMeta {
                retrieval_profile_id: prepared.retrieval_profile_id,
                hits,
                model: prepared.generation_model_name.clone(),
                prompt: prepared.user_prompt.clone(),
                timings: prepared.timings,
            });

            let mut timings = prepared.timings;

            if prepared.retrieved.is_empty() {
                yield ChatSseEvent::Delta(ChatStreamDelta {
                    text: NO_CONTEXT_ANSWER.to_string(),
                });
                timings.total_ms = elapsed_ms(started);
                yield ChatSseEvent::Done(ChatStreamDone { timings });
                return;
            }

            let generate_start = Instant::now();
            let stream_result = generation_service
                .generate_stream(
                    prepared.generation_model_id,
                    GenerationPrompt {
                        system: prepared.system_prompt,
                        user: prepared.user_prompt,
                        temperature: GENERATION_TEMPERATURE,
                        response_format: GenerationResponseFormat::Text,
                    },
                )
                .await;

            let mut token_stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    yield ChatSseEvent::Error(ChatStreamError { message: e.to_string() });
                    return;
                }
            };

            while let Some(item) = token_stream.next().await {
                match item {
                    Ok(token) => {
                        yield ChatSseEvent::Delta(ChatStreamDelta { text: token });
                    }
                    Err(e) => {
                        yield ChatSseEvent::Error(ChatStreamError { message: e.to_string() });
                        return;
                    }
                }
            }

            timings.generate_ms = Some(elapsed_ms(generate_start));
            timings.total_ms = elapsed_ms(started);
            yield ChatSseEvent::Done(ChatStreamDone { timings });
        };

        Ok(s.boxed())
    }

    async fn prepare(&self, req: ChatRequest) -> Result<PreparedChat, AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::Validation("query text is empty".into()));
        }
        let top_k = req.top_k.clamp(1, 50);
        let min_score = req.min_score.clamp(0.0, 1.0);

        let mut timings = Timings::default();

        let profile = self
            .retrieval_profile_resolver
            .resolve(req.retrieval_profile_id)
            .await?;

        let fetch_k = if profile.reranker_model.is_some() {
            rerank_fetch_k(top_k)
        } else {
            top_k
        };

        let embed_start = Instant::now();
        let embeddings = self
            .embedding_service
            .embed_with_resolved(
                &profile.index_profile.embedding_model,
                slice::from_ref(&req.query),
            )
            .await?;
        timings.embed_ms = elapsed_ms(embed_start);
        let query_vector = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("embedder returned no vector for query".into()))?;

        let vector_index = self
            .vector_index_resolver
            .build(&profile.index_profile.vector_index)?;
        let filter: Vec<MetadataFilter> = req
            .metadata_filters
            .iter()
            .filter(|f| !f.field.trim().is_empty() && !f.value.trim().is_empty())
            .map(|f| MetadataFilter {
                field: f.field.trim().to_string(),
                operation: MetadataFilterOperation::Equal,
                value: f.value.trim().to_string(),
            })
            .collect();
        let retrieve_start = Instant::now();
        let matches = vector_index
            .query(&VectorQuery {
                vector: query_vector,
                top_k: fetch_k,
                filter,
            })
            .await?;
        timings.retrieve_ms = elapsed_ms(retrieve_start);

        let matches = match profile.reranker_model.as_ref() {
            Some(reranker) => {
                let rerank_start = Instant::now();
                let matches = self
                    .rerank_service
                    .rerank_matches(reranker.reranker_model_id, &req.query, matches)
                    .await?;
                timings.rerank_ms = Some(elapsed_ms(rerank_start));
                matches
            }
            None => matches,
        };

        let mut retrieved: Vec<RetrievedChunk> = Vec::with_capacity(matches.len());
        for m in matches {
            if m.score < min_score {
                continue;
            }
            if retrieved.len() >= top_k as usize {
                break;
            }
            let meta = m.metadata;
            let document_id = meta
                .get("document_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let chunk_id = meta
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let heading = meta
                .get("heading")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .filter(|s| !s.is_empty());
            let text = meta
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_start = meta
                .get("char_start")
                .and_then(serde_json::Value::as_u64)
                .map(|n| n as u32);
            let char_end = meta
                .get("char_end")
                .and_then(serde_json::Value::as_u64)
                .map(|n| n as u32);

            let (source_ref_key, document_title) = match document_id {
                Some(doc_id) => match self.source_document_repository.load(doc_id).await? {
                    Some(doc) => {
                        let title = doc.latest_metadata.title().to_string();
                        (Some(doc.source_ref.natural_key()), Some(title))
                    }
                    None => (None, None),
                },
                None => (None, None),
            };

            retrieved.push(RetrievedChunk {
                hit: QueryHit {
                    id: m.id,
                    score: m.score,
                    document_id,
                    source_ref_key,
                    document_title,
                    chunk_id,
                    heading,
                    snippet: snippet(&text),
                    char_start,
                    char_end,
                },
                text,
            });
        }

        let user_prompt = build_prompt(&req.query, &retrieved);

        Ok(PreparedChat {
            retrieval_profile_id: req.retrieval_profile_id,
            generation_model_id: profile.generation_model.generation_model_id,
            generation_model_name: profile.generation_model.model.clone(),
            query: req.query,
            retrieved,
            system_prompt: SYSTEM_PROMPT.to_string(),
            user_prompt,
            timings,
        })
    }
}

fn elapsed_ms(start: Instant) -> u32 {
    u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX)
}

fn build_prompt(question: &str, chunks: &[RetrievedChunk]) -> String {
    let mut user = String::from("Excerpts:\n\n");

    for c in chunks {
        let heading = c.hit.heading.as_deref().unwrap_or("(untitled)");
        user.push_str("=== EXCERPT ===\nTitle: ");
        user.push_str(heading);
        user.push_str("\nContent:\n");
        user.push_str(&c.text);
        user.push_str("\n\n");
    }

    user.push_str("Question: ");
    user.push_str(question);
    user.push_str("\n\nAnswer:");
    user
}

fn snippet(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= SNIPPET_MAX_CHARS {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(SNIPPET_MAX_CHARS).collect();
    out.push('…');
    out
}
