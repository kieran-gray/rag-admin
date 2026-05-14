use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ChunkStrategy {
    Bert,
    #[default]
    Section,
    Llm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkParamKey {
    MaxSectionTokens,
    TargetTokens,
    OverlapTokens,
    MinTokens,
    LlmMicroChunkTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkParamDefinition {
    pub key: ChunkParamKey,
    pub label: &'static str,
    pub hint: &'static str,
    pub min: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkerDefinition {
    pub strategy: ChunkStrategy,
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub params: &'static [ChunkParamDefinition],
}

const SECTION_PARAMS: &[ChunkParamDefinition] = &[ChunkParamDefinition {
    key: ChunkParamKey::MaxSectionTokens,
    label: "MAX_SECTION_TOKENS",
    hint: "section: max tokens per chunk before fallback split",
    min: 1,
}];

const BERT_PARAMS: &[ChunkParamDefinition] = &[
    ChunkParamDefinition {
        key: ChunkParamKey::TargetTokens,
        label: "TARGET_TOKENS",
        hint: "bert: target chunk size in tokens",
        min: 1,
    },
    ChunkParamDefinition {
        key: ChunkParamKey::OverlapTokens,
        label: "OVERLAP_TOKENS",
        hint: "bert: tokens of overlap between adjacent chunks",
        min: 0,
    },
    ChunkParamDefinition {
        key: ChunkParamKey::MinTokens,
        label: "MIN_TOKENS",
        hint: "bert: small trailing chunks merge with the previous one",
        min: 0,
    },
];

const LLM_PARAMS: &[ChunkParamDefinition] = &[
    ChunkParamDefinition {
        key: ChunkParamKey::TargetTokens,
        label: "TARGET_TOKENS",
        hint: "llm: maximum final chunk size in tokens",
        min: 1,
    },
    ChunkParamDefinition {
        key: ChunkParamKey::LlmMicroChunkTokens,
        label: "MICRO_CHUNK_TOKENS",
        hint: "llm: punctuation-aware micro chunks offered to the model for boundary selection",
        min: 32,
    },
];

const CHUNKER_DEFINITIONS: &[ChunkerDefinition] = &[
    ChunkerDefinition {
        strategy: ChunkStrategy::Bert,
        id: "bert",
        label: "bert",
        hint: "sliding window with overlap",
        params: BERT_PARAMS,
    },
    ChunkerDefinition {
        strategy: ChunkStrategy::Section,
        id: "section",
        label: "section",
        hint: "heading-aware markdown sections",
        params: SECTION_PARAMS,
    },
    ChunkerDefinition {
        strategy: ChunkStrategy::Llm,
        id: "llm",
        label: "llm",
        hint: "LLM-selected semantic boundaries over micro chunks",
        params: LLM_PARAMS,
    },
];

impl ChunkStrategy {
    pub fn all() -> &'static [ChunkerDefinition] {
        CHUNKER_DEFINITIONS
    }

    pub fn as_str(self) -> &'static str {
        self.definition().id
    }

    pub fn from_id(value: &str) -> Option<Self> {
        CHUNKER_DEFINITIONS
            .iter()
            .find(|definition| definition.id == value)
            .map(|definition| definition.strategy)
    }

    pub fn definition(self) -> &'static ChunkerDefinition {
        CHUNKER_DEFINITIONS
            .iter()
            .find(|definition| definition.strategy == self)
            .expect("chunk strategy has a definition")
    }

    pub fn preview_limit_uses_tokens(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingConfig {
    Bert(BertChunkingConfig),
    Section(SectionChunkingConfig),
    Llm(LlmChunkingConfig),
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
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub overlap_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
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
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
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
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
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
        }
    }

    pub fn for_strategy(strategy: ChunkStrategy) -> Self {
        match strategy {
            ChunkStrategy::Bert => Self::Bert(BertChunkingConfig::default()),
            ChunkStrategy::Section => Self::Section(SectionChunkingConfig::default()),
            ChunkStrategy::Llm => Self::Llm(LlmChunkingConfig::default()),
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
            _ => {}
        }
    }

    pub fn size_limit_for_display(&self, token_limit: u32) -> u32 {
        match self {
            Self::Bert(c) => c.target_tokens.min(token_limit),
            Self::Section(c) => c.max_section_tokens.min(token_limit),
            Self::Llm(c) => c.target_tokens.min(token_limit),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            Self::Bert(config) => {
                format!("bert:{}/{}", config.target_tokens, config.overlap_tokens)
            }
            Self::Section(config) => format!("section:{}", config.max_section_tokens),
            Self::Llm(config) => format!("llm:{}", config.micro_chunk_tokens),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_strategy_definition_round_trips_by_id() {
        for definition in ChunkStrategy::all() {
            assert_eq!(
                ChunkStrategy::from_id(definition.id),
                Some(definition.strategy)
            );
            assert_eq!(definition.strategy.as_str(), definition.id);
        }
    }

    #[test]
    fn every_strategy_has_editable_params() {
        for definition in ChunkStrategy::all() {
            assert!(
                !definition.params.is_empty(),
                "{} has no params",
                definition.id
            );
        }
    }
}
