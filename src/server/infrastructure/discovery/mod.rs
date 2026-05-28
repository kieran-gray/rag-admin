pub mod cloudflare;
pub mod cloudflare_vectorize;
pub mod ollama;

pub use cloudflare::CloudflareDiscoveryAdapter;
pub use cloudflare_vectorize::CloudflareVectorizeDiscoveryAdapter;
pub use ollama::OllamaDiscoveryAdapter;
