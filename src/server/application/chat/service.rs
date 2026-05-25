use std::slice;
use std::sync::Arc;

use uuid::Uuid;

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
use crate::shared::contracts::{ChatRequest, ChatResponse, QueryHit};

const SYSTEM_PROMPT: &str = include_str!("prompts/chat_system_prompt.txt");
const SNIPPET_MAX_CHARS: usize = 320;
const GENERATION_TEMPERATURE: f32 = 0.2;

struct RetrievedChunk {
    hit: QueryHit,
    text: String,
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
        if req.query.trim().is_empty() {
            return Err(AppError::Validation("query text is empty".into()));
        }
        let top_k = req.top_k.clamp(1, 50);
        let min_score = req.min_score.clamp(0.0, 1.0);

        let profile = self
            .retrieval_profile_resolver
            .resolve(req.retrieval_profile_id)
            .await?;

        let fetch_k = if profile.reranker_model.is_some() {
            rerank_fetch_k(top_k)
        } else {
            top_k
        };

        let embeddings = self
            .embedding_service
            .embed_with_resolved(
                &profile.index_profile.embedding_model,
                slice::from_ref(&req.query),
            )
            .await?;
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
        let matches = vector_index
            .query(&VectorQuery {
                vector: query_vector,
                top_k: fetch_k,
                filter,
            })
            .await?;

        let matches = match profile.reranker_model.as_ref() {
            Some(reranker) => {
                self.rerank_service
                    .rerank_matches(reranker.reranker_model_id, &req.query, matches)
                    .await?
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

        let answer = if retrieved.is_empty() {
            "I don't see that in the indexed documents.".to_string()
        } else {
            let prompt = build_prompt(&req.query, &retrieved);
            let response = self
                .generation_service
                .generate(
                    profile.generation_model.generation_model_id,
                    GenerationPrompt {
                        system: SYSTEM_PROMPT.to_string(),
                        user: prompt,
                        temperature: GENERATION_TEMPERATURE,
                        response_format: GenerationResponseFormat::Text,
                    },
                )
                .await?;
            response.content
        };

        Ok(ChatResponse {
            retrieval_profile_id: req.retrieval_profile_id,
            query: req.query,
            answer,
            model: profile.generation_model.model,
            hits: retrieved.into_iter().map(|r| r.hit).collect(),
        })
    }
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
