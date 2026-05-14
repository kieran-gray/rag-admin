use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;

use crate::server::application::indexing::ports::vector_index::{
    VectorIndex, VectorIndexDescription, VectorMatch, VectorQuery,
};
use crate::server::application::AppError;
use crate::server::domain::VectorRecord;
use crate::server::infrastructure::clients::{CloudflareApi, CLOUDFLARE_API_BASE};

pub struct NamedVectorizeIndex {
    api: Arc<CloudflareApi>,
    index_name: String,
}

impl NamedVectorizeIndex {
    pub fn new(api: Arc<CloudflareApi>, index_name: String) -> Arc<Self> {
        Arc::new(Self { api, index_name })
    }

    fn url(&self, operation: &str) -> String {
        format!(
            "{}/accounts/{}/vectorize/v2/indexes/{}/{}",
            CLOUDFLARE_API_BASE,
            self.api.account_id(),
            self.index_name,
            operation
        )
    }
}

#[derive(Deserialize)]
struct QueryEnvelope {
    result: Option<QueryResult>,
}

#[derive(Deserialize)]
struct QueryResult {
    matches: Vec<QueryMatch>,
}

#[derive(Deserialize)]
struct QueryMatch {
    id: String,
    score: f32,
    metadata: Option<Value>,
}

#[derive(Deserialize)]
struct DescribeEnvelope {
    result: Option<DescribeResult>,
}

#[derive(Deserialize)]
struct DescribeResult {
    name: String,
    config: DescribeConfig,
}

#[derive(Deserialize)]
struct DescribeConfig {
    dimensions: u32,
}

#[async_trait]
impl VectorIndex for NamedVectorizeIndex {
    async fn upsert(&self, records: &[VectorRecord]) -> Result<(), AppError> {
        if records.is_empty() {
            return Ok(());
        }
        let url = self.url("upsert");
        let mut ndjson = String::new();
        for r in records {
            let line = serde_json::to_string(r)
                .map_err(|e| AppError::Internal(format!("encode vector record: {e}")))?;
            ndjson.push_str(&line);
            ndjson.push('\n');
        }
        self.api
            .request(
                Method::POST,
                &url,
                ndjson.into_bytes(),
                "application/x-ndjson",
                "vectorize-upsert",
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, ids: &[String]) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        let url = self.url("delete-by-ids");
        let body = serde_json::json!({ "ids": ids });
        let bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode delete body: {e}")))?;
        self.api
            .request(
                Method::POST,
                &url,
                bytes,
                "application/json",
                "vectorize-delete",
            )
            .await?;
        Ok(())
    }

    async fn query(&self, q: &VectorQuery) -> Result<Vec<VectorMatch>, AppError> {
        let url = self.url("query");
        let body = serde_json::json!({
            "vector": q.vector,
            "topK": q.top_k,
            "returnMetadata": "all",
        });
        let bytes = serde_json::to_vec(&body)
            .map_err(|e| AppError::Internal(format!("encode query body: {e}")))?;
        let response = self
            .api
            .request(
                Method::POST,
                &url,
                bytes,
                "application/json",
                "vectorize-query",
            )
            .await?;
        let envelope: QueryEnvelope = serde_json::from_str(&response)
            .map_err(|e| AppError::Internal(format!("query response parse: {e}")))?;
        Ok(envelope
            .result
            .map(|r| r.matches)
            .unwrap_or_default()
            .into_iter()
            .map(|m| VectorMatch {
                id: m.id,
                score: m.score,
                metadata: m.metadata.unwrap_or(Value::Null),
            })
            .collect())
    }

    async fn describe(&self) -> Result<VectorIndexDescription, AppError> {
        let url = self.url("info");
        let response = self
            .api
            .request(
                Method::GET,
                &url,
                vec![],
                "application/json",
                "vectorize-describe",
            )
            .await?;
        let envelope: DescribeEnvelope = serde_json::from_str(&response)
            .map_err(|e| AppError::Internal(format!("describe response parse: {e}")))?;
        let result = envelope
            .result
            .ok_or_else(|| AppError::Internal("describe returned no result".into()))?;
        Ok(VectorIndexDescription {
            name: result.name,
            dimensions: result.config.dimensions,
        })
    }
}
