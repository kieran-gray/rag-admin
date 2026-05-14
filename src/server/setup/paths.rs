use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    std::env::current_dir()
        .map(|p| p.join("rag-admin").join("data"))
        .unwrap_or_else(|_| PathBuf::from("rag-admin/data"))
}

pub fn tokenizer_path() -> PathBuf {
    data_dir().join("tokenizer.json")
}

pub fn post_chunking_config_path() -> PathBuf {
    data_dir().join("post-chunking.json")
}

pub fn evaluation_defaults_path() -> PathBuf {
    data_dir().join("evaluation-defaults.json")
}
