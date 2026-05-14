use async_trait::async_trait;
use serde_json::Value;

use crate::server::application::AppError;
use crate::server::domain::VectorRecord;

pub enum MetadataFilterOperation {
    Equal,
    NotEqual,
}

pub struct MetadataFilter {
    pub field: String,
    pub operation: MetadataFilterOperation,
    pub value: String,
}

pub struct VectorQuery {
    pub vector: Vec<f32>,
    pub top_k: u32,
    pub filter: Vec<MetadataFilter>,
}

pub struct VectorMatch {
    pub id: String,
    pub score: f32,
    pub metadata: Value,
}

pub struct VectorIndexDescription {
    pub name: String,
    pub dimensions: u32,
}

#[async_trait]
pub trait VectorIndex: Send + Sync {
    async fn upsert(&self, records: &[VectorRecord]) -> Result<(), AppError>;
    async fn delete(&self, ids: &[String]) -> Result<(), AppError>;
    async fn query(&self, q: &VectorQuery) -> Result<Vec<VectorMatch>, AppError>;
    async fn describe(&self) -> Result<VectorIndexDescription, AppError>;
}
