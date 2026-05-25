pub mod llama_server_generation_client;
pub mod ollama_generation_client;
pub mod workers_ai_generation_client;

pub use llama_server_generation_client::LlamaServerGenerationClient;
pub use ollama_generation_client::OllamaGenerationClient;
pub use workers_ai_generation_client::WorkersAiGenerationClient;
