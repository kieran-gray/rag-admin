use std::{collections::BTreeMap, sync::Arc};

use crate::{
    server::application::{
        chunking::{ports::DocumentChunker, ChunkOutput},
        ports::{MarkdownParser, Tokenizer},
        AppError,
    },
    shared::{ChunkStrategy, ChunkingConfig},
};

pub struct ChunkerRegistry {
    chunkers: BTreeMap<ChunkStrategy, Arc<dyn DocumentChunker>>,
    markdown_parser: Arc<dyn MarkdownParser>,
    tokenizer: Arc<dyn Tokenizer>,
}

impl ChunkerRegistry {
    pub fn new(tokenizer: Arc<dyn Tokenizer>, markdown_parser: Arc<dyn MarkdownParser>) -> Self {
        Self {
            chunkers: BTreeMap::new(),
            markdown_parser,
            tokenizer,
        }
    }

    pub fn add(&mut self, chunker: Arc<dyn DocumentChunker>) {
        self.chunkers.insert(chunker.strategy(), chunker);
    }

    pub async fn chunk_markdown(
        &self,
        config: &ChunkingConfig,
        source: &str,
    ) -> Result<Vec<ChunkOutput>, AppError> {
        let strategy = config.strategy();
        let chunker = self.chunkers.get(&strategy).ok_or_else(|| {
            AppError::Validation(format!(
                "unsupported chunking strategy '{}'",
                strategy.as_str()
            ))
        })?;

        let markdown = self.markdown_parser.parse(source)?;

        let chunks = chunker
            .chunk(config, &markdown, self.tokenizer.as_ref())
            .await?;
        Ok(chunks)
    }
}
