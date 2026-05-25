pub mod cloudflare;
pub mod llama_server;
pub mod ollama;

pub use cloudflare::{CloudflareApi, CLOUDFLARE_API_BASE};
pub use llama_server::LlamaServerApi;
pub use ollama::OllamaApi;
