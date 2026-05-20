use std::sync::Arc;

use tiktoken_rs::{bpe_for_model, CoreBPE};

use crate::server::application::ports::{Tokenized, Tokenizer};
use crate::server::application::AppError;

pub const DEFAULT_CHUNK_TOKEN_LIMIT: u32 = 512;

pub const DEFAULT_TIKTOKEN_MODEL: &str = "gpt-4o-mini";

pub struct TiktokenTokenizer {
    bpe: CoreBPE,
    model: String,
}

impl TiktokenTokenizer {
    pub fn for_model(model: &str) -> Result<Arc<Self>, AppError> {
        let bpe = bpe_for_model(model)
            .map_err(|e| AppError::Internal(format!("load tiktoken model '{model}': {e}")))?;
        Ok(Arc::new(Self {
            bpe: bpe.clone(),
            model: model.to_string(),
        }))
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl Tokenizer for TiktokenTokenizer {
    fn encode(&self, text: &str) -> Result<Tokenized, AppError> {
        let ids = self.bpe.encode_ordinary(text);
        let mut tokens = Vec::with_capacity(ids.len());
        for id in &ids {
            let piece = self.bpe.decode(&[*id]).unwrap_or_else(|_| String::new());
            tokens.push(piece);
        }
        let count = u32::try_from(ids.len())
            .map_err(|e| AppError::Internal(format!("token count: {e}")))?;
        Ok(Tokenized { tokens, count })
    }

    fn count(&self, text: &str) -> Result<u32, AppError> {
        let ids = self.bpe.encode_ordinary(text);
        u32::try_from(ids.len()).map_err(|e| AppError::Internal(format!("token count: {e}")))
    }

    fn token_byte_offsets(&self, text: &str) -> Result<Vec<usize>, AppError> {
        let ids = self.bpe.encode_ordinary(text);
        let mut offsets = Vec::with_capacity(ids.len());
        let mut cursor = 0usize;
        let text_len = text.len();

        for id in ids {
            offsets.push(cursor.min(text_len));
            match self.bpe.decode(&[id]) {
                Ok(piece) => cursor = cursor.saturating_add(piece.len()),
                Err(_) => {
                    // Mid-codepoint BPE token; advance one byte and let later
                    // tokens realign. Adequate for darn's chunk-size budgeting.
                    cursor = cursor.saturating_add(1);
                }
            }
        }

        Ok(offsets)
    }
}
