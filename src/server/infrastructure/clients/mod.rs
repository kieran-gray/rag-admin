pub mod cloudflare;
pub mod ollama;

pub use cloudflare::{CloudflareApi, CLOUDFLARE_API_BASE};
pub use ollama::OllamaApi;
