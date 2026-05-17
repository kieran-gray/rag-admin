use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ChunkStrategy {
    Bert,
    #[default]
    Section,
    Llm,
    Darn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkParamKey {
    MaxSectionTokens,
    TargetTokens,
    OverlapTokens,
    MinTokens,
    LlmMicroChunkTokens,
    DarnMaxChunkSize,
    DarnOverlap,
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

const DARN_PARAMS: &[ChunkParamDefinition] = &[
    ChunkParamDefinition {
        key: ChunkParamKey::DarnMaxChunkSize,
        label: "MAX_CHUNK_SIZE",
        hint: "darn: maximum chunk size in characters or tokens (set at config time)",
        min: 1,
    },
    ChunkParamDefinition {
        key: ChunkParamKey::DarnOverlap,
        label: "OVERLAP",
        hint: "darn: characters or tokens repeated at chunk boundaries",
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

const BERT_DEFINITION: ChunkerDefinition = ChunkerDefinition {
    strategy: ChunkStrategy::Bert,
    id: "bert",
    label: "bert",
    hint: "sliding window with overlap",
    params: BERT_PARAMS,
};

const SECTION_DEFINITION: ChunkerDefinition = ChunkerDefinition {
    strategy: ChunkStrategy::Section,
    id: "section",
    label: "section",
    hint: "heading-aware markdown sections",
    params: SECTION_PARAMS,
};

const LLM_DEFINITION: ChunkerDefinition = ChunkerDefinition {
    strategy: ChunkStrategy::Llm,
    id: "llm",
    label: "llm",
    hint: "LLM-selected semantic boundaries over micro chunks",
    params: LLM_PARAMS,
};

const DARN_DEFINITION: ChunkerDefinition = ChunkerDefinition {
    strategy: ChunkStrategy::Darn,
    id: "darn",
    label: "darn",
    hint: "mathematically optimal cuts via DP over markdown structure penalties",
    params: DARN_PARAMS,
};

pub const CHUNKER_DEFINITIONS: &[ChunkerDefinition] = &[
    BERT_DEFINITION,
    SECTION_DEFINITION,
    LLM_DEFINITION,
    DARN_DEFINITION,
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
        match self {
            ChunkStrategy::Bert => &BERT_DEFINITION,
            ChunkStrategy::Section => &SECTION_DEFINITION,
            ChunkStrategy::Llm => &LLM_DEFINITION,
            ChunkStrategy::Darn => &DARN_DEFINITION,
        }
    }

    pub fn preview_limit_uses_tokens(self) -> bool {
        true
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
