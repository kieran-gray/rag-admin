use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::configuration::PipelineResolver;
use crate::server::application::embedding::EmbeddingService;
use crate::server::application::indexing::ports::vector_index::VectorQuery;
use crate::server::application::indexing::VectorIndexResolver;
use crate::server::application::AppError;
use crate::server::domain::source_document::repository::SourceDocumentRepository;
use crate::server::domain::source_document::version::DocumentMetadata;
use crate::shared::{QueryHit, QueryRequest, QueryResult};

const SNIPPET_MAX_CHARS: usize = 320;

pub struct QueryService {
    pipeline_resolver: Arc<PipelineResolver>,
    embedding_service: Arc<EmbeddingService>,
    vector_index_resolver: Arc<VectorIndexResolver>,
    source_document_repository: Arc<dyn SourceDocumentRepository>,
}

impl QueryService {
    pub fn new(
        pipeline_resolver: Arc<PipelineResolver>,
        embedding_service: Arc<EmbeddingService>,
        vector_index_resolver: Arc<VectorIndexResolver>,
        source_document_repository: Arc<dyn SourceDocumentRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pipeline_resolver,
            embedding_service,
            vector_index_resolver,
            source_document_repository,
        })
    }

    pub async fn query(&self, req: QueryRequest) -> Result<QueryResult, AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::Validation("query text is empty".into()));
        }
        let top_k = req.top_k.clamp(1, 50);
        let min_score = req.min_score.clamp(0.0, 1.0);

        let pipeline = self
            .pipeline_resolver
            .resolve(req.pipeline_configuration_id)
            .await?;

        let embeddings = self
            .embedding_service
            .embed_with_resolved(&pipeline.embedding_model, std::slice::from_ref(&req.query))
            .await?;
        let query_vector = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Internal("embedder returned no vector for query".into()))?;

        let vector_index = self.vector_index_resolver.build(&pipeline.vector_index)?;
        let matches = vector_index
            .query(&VectorQuery {
                vector: query_vector,
                top_k,
                filter: Vec::new(),
            })
            .await?;

        let mut hits = Vec::with_capacity(matches.len());
        for m in matches {
            if m.score < min_score {
                continue;
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
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let text = meta
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_start = meta
                .get("char_start")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let char_end = meta
                .get("char_end")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);

            let (source_ref_key, document_title) = match document_id {
                Some(doc_id) => match self.source_document_repository.load(doc_id).await? {
                    Some(doc) => {
                        let title = match &doc.latest_metadata {
                            DocumentMetadata::BlogPost(m) => m.title.clone(),
                        };
                        (Some(doc.source_ref.natural_key().to_string()), Some(title))
                    }
                    None => (None, None),
                },
                None => (None, None),
            };

            hits.push(QueryHit {
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
            });
        }

        Ok(QueryResult {
            pipeline_configuration_id: req.pipeline_configuration_id,
            query: req.query,
            hits,
        })
    }
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
