use std::path::PathBuf;
use std::sync::Arc;

use tokenizers::Tokenizer as HfInner;
use tokio::fs;

use crate::server::application::ports::{Tokenized, Tokenizer};
use crate::server::application::AppError;
use crate::server::infrastructure::http_client::ReqwestHttpClient;

const TOKENIZER_URL: &str =
    "https://huggingface.co/BAAI/bge-base-en-v1.5/resolve/main/tokenizer.json";

pub const EMBEDDING_TOKEN_LIMIT: u32 = 512;

pub struct HuggingFaceTokenizer {
    inner: HfInner,
}

impl HuggingFaceTokenizer {
    pub async fn load_or_fetch(
        cache_path: PathBuf,
        http: Arc<ReqwestHttpClient>,
    ) -> Result<Arc<Self>, AppError> {
        if !cache_path.exists() {
            if let Some(parent) = cache_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AppError::Io(format!("create tokenizer dir: {e}")))?;
            }
            let bytes = http
                .raw()
                .get(TOKENIZER_URL)
                .send()
                .await
                .map_err(|e| AppError::Upstream(format!("fetch tokenizer: {e}")))?
                .error_for_status()
                .map_err(|e| AppError::Upstream(format!("fetch tokenizer status: {e}")))?
                .bytes()
                .await
                .map_err(|e| AppError::Upstream(format!("read tokenizer body: {e}")))?;
            fs::write(&cache_path, &bytes)
                .await
                .map_err(|e| AppError::Io(format!("write tokenizer cache: {e}")))?;
        }

        let inner = HfInner::from_file(&cache_path)
            .map_err(|e| AppError::Internal(format!("load tokenizer: {e}")))?;
        Ok(Arc::new(Self { inner }))
    }
}

impl Tokenizer for HuggingFaceTokenizer {
    fn encode(&self, text: &str) -> Result<Tokenized, AppError> {
        let encoding = self
            .inner
            .encode(text, true)
            .map_err(|e| AppError::Internal(format!("tokenize: {e}")))?;
        let tokens = encoding.get_tokens().to_vec();
        let count = u32::try_from(tokens.len()).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Tokenized { tokens, count })
    }
}
