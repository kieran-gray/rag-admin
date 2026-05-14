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
}
