use crate::server::application::ports::Tokenizer;
use crate::server::application::AppError;

use super::dp::cheapest_path_indices;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Characters,
    Tokens,
}

/// Wraps the per-byte punishment vector and (when running in token mode) a
/// byte→token-boundary mapping derived from the project's `Tokenizer` port.
/// Construction is fallible because the token alignment can fail if the
/// tokenizer errors out.
pub struct ChunkOptimiser {
    punishments: Vec<usize>,
    granularity: Granularity,
    /// Byte offsets of each token's first byte. Only populated for `Tokens`.
    token_start_byte_idx: Vec<usize>,
}

impl ChunkOptimiser {
    pub fn new(
        text: &str,
        punishments: Vec<usize>,
        granularity: Granularity,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Self, AppError> {
        let token_start_byte_idx = match granularity {
            Granularity::Characters => Vec::new(),
            Granularity::Tokens => tokenizer.token_byte_offsets(text)?,
        };

        Ok(Self {
            punishments,
            granularity,
            token_start_byte_idx,
        })
    }

    /// Compute optimal chunk-start byte offsets for the configured granularity.
    pub fn optimise_chunks(&self, max_chunk_size: usize) -> Result<Vec<usize>, AppError> {
        match self.granularity {
            Granularity::Characters => cheapest_path_indices(&self.punishments, max_chunk_size),
            Granularity::Tokens => {
                if self.token_start_byte_idx.is_empty() {
                    return Ok(vec![0]);
                }
                let reduced = self.build_reduced_vector();
                let token_path = cheapest_path_indices(&reduced, max_chunk_size)?;
                Ok(token_path
                    .into_iter()
                    .map(|tok_idx| self.token_start_byte_idx[tok_idx])
                    .collect())
            }
        }
    }

    fn build_reduced_vector(&self) -> Vec<usize> {
        self.token_start_byte_idx
            .iter()
            .map(|&i| self.punishments.get(i).copied().unwrap_or(0))
            .collect()
    }
}
