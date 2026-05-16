use crate::server::application::ports::{Tokenized, Tokenizer};
use crate::server::application::AppError;

pub struct MockTokenizer;

impl MockTokenizer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer for MockTokenizer {
    fn encode(&self, text: &str) -> Result<Tokenized, AppError> {
        let tokens: Vec<String> = text.split_whitespace().map(|s| s.to_string()).collect();
        let count = tokens.len() as u32;
        Ok(Tokenized { tokens, count })
    }

    fn token_byte_offsets(&self, text: &str) -> Result<Vec<usize>, AppError> {
        let bytes = text.as_bytes();
        let mut offsets = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            offsets.push(i);
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
        }
        Ok(offsets)
    }
}
