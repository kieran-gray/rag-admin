use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::chunkers::{ChunkParamKey, ChunkStrategy};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingConfig {
    Bert(BertChunkingConfig),
    Section(SectionChunkingConfig),
    Llm(LlmChunkingConfig),
    Darn(DarnChunkingConfig),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DarnGranularity {
    #[default]
    Characters,
    Tokens,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DarnChunkingConfig {
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub max_chunk_size: u32,
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub overlap: u32,
    #[serde(default)]
    pub granularity: DarnGranularity,
}

impl Default for DarnChunkingConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 500,
            overlap: 50,
            granularity: DarnGranularity::Characters,
        }
    }
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self::Section(SectionChunkingConfig {
            max_section_tokens: 2000,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BertChunkingConfig {
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub overlap_tokens: u32,
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub min_tokens: u32,
}

impl Default for BertChunkingConfig {
    fn default() -> Self {
        Self {
            target_tokens: 384,
            overlap_tokens: 64,
            min_tokens: 96,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionChunkingConfig {
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub max_section_tokens: u32,
}

impl Default for SectionChunkingConfig {
    fn default() -> Self {
        Self {
            max_section_tokens: 480,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmChunkingConfig {
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::core::serde_compat::u32_from_string")]
    pub micro_chunk_tokens: u32,
    pub generation_model_id: Uuid,
}

impl Default for LlmChunkingConfig {
    fn default() -> Self {
        Self {
            target_tokens: 384,
            micro_chunk_tokens: 96,
            generation_model_id: Uuid::nil(),
        }
    }
}

impl ChunkingConfig {
    pub fn strategy(&self) -> ChunkStrategy {
        match self {
            Self::Bert(_) => ChunkStrategy::Bert,
            Self::Section(_) => ChunkStrategy::Section,
            Self::Llm(_) => ChunkStrategy::Llm,
            Self::Darn(_) => ChunkStrategy::Darn,
        }
    }

    pub fn for_strategy(strategy: ChunkStrategy) -> Self {
        match strategy {
            ChunkStrategy::Bert => Self::Bert(BertChunkingConfig::default()),
            ChunkStrategy::Section => Self::Section(SectionChunkingConfig::default()),
            ChunkStrategy::Llm => Self::Llm(LlmChunkingConfig::default()),
            ChunkStrategy::Darn => Self::Darn(DarnChunkingConfig::default()),
        }
    }

    pub fn param_value(&self, key: ChunkParamKey) -> u32 {
        match (self, key) {
            (Self::Section(c), ChunkParamKey::MaxSectionTokens) => c.max_section_tokens,
            (Self::Bert(c), ChunkParamKey::TargetTokens) => c.target_tokens,
            (Self::Bert(c), ChunkParamKey::OverlapTokens) => c.overlap_tokens,
            (Self::Bert(c), ChunkParamKey::MinTokens) => c.min_tokens,
            (Self::Llm(c), ChunkParamKey::TargetTokens) => c.target_tokens,
            (Self::Llm(c), ChunkParamKey::LlmMicroChunkTokens) => c.micro_chunk_tokens,
            (Self::Darn(c), ChunkParamKey::DarnMaxChunkSize) => c.max_chunk_size,
            (Self::Darn(c), ChunkParamKey::DarnOverlap) => c.overlap,
            _ => 0,
        }
    }

    pub fn set_param_value(&mut self, key: ChunkParamKey, value: u32) {
        match (self, key) {
            (Self::Section(c), ChunkParamKey::MaxSectionTokens) => c.max_section_tokens = value,
            (Self::Bert(c), ChunkParamKey::TargetTokens) => c.target_tokens = value,
            (Self::Bert(c), ChunkParamKey::OverlapTokens) => c.overlap_tokens = value,
            (Self::Bert(c), ChunkParamKey::MinTokens) => c.min_tokens = value,
            (Self::Llm(c), ChunkParamKey::TargetTokens) => c.target_tokens = value,
            (Self::Llm(c), ChunkParamKey::LlmMicroChunkTokens) => c.micro_chunk_tokens = value,
            (Self::Darn(c), ChunkParamKey::DarnMaxChunkSize) => c.max_chunk_size = value,
            (Self::Darn(c), ChunkParamKey::DarnOverlap) => c.overlap = value,
            _ => {}
        }
    }

    pub fn size_limit_for_display(&self, token_limit: u32) -> u32 {
        match self {
            Self::Bert(c) => c.target_tokens.min(token_limit),
            Self::Section(c) => c.max_section_tokens.min(token_limit),
            Self::Llm(c) => c.target_tokens.min(token_limit),
            Self::Darn(c) => c.max_chunk_size.min(token_limit),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            Self::Bert(config) => {
                format!("bert:{}/{}", config.target_tokens, config.overlap_tokens)
            }
            Self::Section(config) => format!("section:{}", config.max_section_tokens),
            Self::Llm(config) => format!("llm:{}", config.micro_chunk_tokens),
            Self::Darn(config) => format!("darn:{}/{}", config.max_chunk_size, config.overlap),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::Bert(config) => format!(
                "bert · target={} · overlap={} · min={}",
                config.target_tokens, config.overlap_tokens, config.min_tokens
            ),
            Self::Section(config) => format!("section · max_tokens={}", config.max_section_tokens),
            Self::Llm(config) => {
                format!("llm · micro_chunk_tokens={}", config.micro_chunk_tokens)
            }
            Self::Darn(config) => format!(
                "darn · max_chunk_size={} · overlap={} · granularity={}",
                config.max_chunk_size,
                config.overlap,
                config.granularity.as_str()
            ),
        }
    }

    pub fn detail_label(&self, size_limit: u32) -> String {
        match self {
            Self::Bert(config) => format!(
                "STRATEGY: BERT · TOKEN_LIMIT: {} · TARGET: {} · OVERLAP: {} · MIN: {}",
                size_limit, config.target_tokens, config.overlap_tokens, config.min_tokens
            ),
            Self::Section(config) => format!(
                "STRATEGY: SECTION · MAX_TOKENS: {}",
                config.max_section_tokens
            ),
            Self::Llm(config) => format!(
                "STRATEGY: LLM · TOKEN_LIMIT: {} · TARGET: {} · MICRO_CHUNK_TOKENS: {}",
                size_limit, config.target_tokens, config.micro_chunk_tokens
            ),
            Self::Darn(config) => format!(
                "STRATEGY: DARN · MAX_CHUNK_SIZE: {} · OVERLAP: {} · GRANULARITY: {}",
                config.max_chunk_size,
                config.overlap,
                config.granularity.as_str().to_uppercase()
            ),
        }
    }
}

impl DarnGranularity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Characters => "characters",
            Self::Tokens => "tokens",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "characters" => Some(Self::Characters),
            "tokens" => Some(Self::Tokens),
            _ => None,
        }
    }
}
