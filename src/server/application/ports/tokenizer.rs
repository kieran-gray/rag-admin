use crate::server::application::AppError;

#[derive(Debug, Clone)]
pub struct Tokenized {
    pub tokens: Vec<String>,
    pub count: u32,
}

pub trait Tokenizer: Send + Sync {
    fn encode(&self, text: &str) -> Result<Tokenized, AppError>;

    fn count(&self, text: &str) -> Result<u32, AppError> {
        self.encode(text).map(|tokenized| tokenized.count)
    }

    /// Byte offset within `text` of each token's first byte. The returned slice
    /// has length equal to the token count. `darn`'s chunk optimiser uses this
    /// to map token-granularity DP indices back onto byte indices.
    fn token_byte_offsets(&self, text: &str) -> Result<Vec<usize>, AppError>;
}
