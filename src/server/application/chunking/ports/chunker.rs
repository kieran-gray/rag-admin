use async_trait::async_trait;

use crate::{
    catalog::ChunkStrategy,
    core::ChunkingConfig,
    server::application::{markdown::Document, ports::Tokenizer, AppError},
};

#[derive(Debug, Clone)]
pub struct ChunkOutput {
    pub chunk_id: u32,
    pub heading: String,
    pub text: String,
    pub char_start: u32,
    pub char_end: u32,
}

#[async_trait]
pub trait DocumentChunker: Send + Sync {
    fn strategy(&self) -> ChunkStrategy;

    async fn chunk(
        &self,
        config: &ChunkingConfig,
        source: &Document,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<ChunkOutput>, AppError>;
}
