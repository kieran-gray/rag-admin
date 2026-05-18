use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{
    BertChunkingConfig, ChunkingConfig, DarnChunkingConfig, DarnGranularity, LlmChunkingConfig,
    SectionChunkingConfig,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingConfigPayload {
    Bert(BertChunkingConfigPayload),
    Section(SectionChunkingConfigPayload),
    Llm(LlmChunkingConfigPayload),
    Darn(DarnChunkingConfigPayload),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BertChunkingConfigPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub overlap_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub min_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionChunkingConfigPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub max_section_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmChunkingConfigPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub micro_chunk_tokens: u32,
    pub generation_model_id: Uuid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DarnChunkingConfigPayload {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub max_chunk_size: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub overlap: u32,
    #[serde(default)]
    pub granularity: DarnGranularityPayload,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DarnGranularityPayload {
    #[default]
    Characters,
    Tokens,
}

impl From<ChunkingConfig> for ChunkingConfigPayload {
    fn from(value: ChunkingConfig) -> Self {
        match value {
            ChunkingConfig::Bert(c) => Self::Bert(c.into()),
            ChunkingConfig::Section(c) => Self::Section(c.into()),
            ChunkingConfig::Llm(c) => Self::Llm(c.into()),
            ChunkingConfig::Darn(c) => Self::Darn(c.into()),
        }
    }
}

impl From<ChunkingConfigPayload> for ChunkingConfig {
    fn from(value: ChunkingConfigPayload) -> Self {
        match value {
            ChunkingConfigPayload::Bert(c) => Self::Bert(c.into()),
            ChunkingConfigPayload::Section(c) => Self::Section(c.into()),
            ChunkingConfigPayload::Llm(c) => Self::Llm(c.into()),
            ChunkingConfigPayload::Darn(c) => Self::Darn(c.into()),
        }
    }
}

impl From<BertChunkingConfig> for BertChunkingConfigPayload {
    fn from(c: BertChunkingConfig) -> Self {
        Self {
            target_tokens: c.target_tokens,
            overlap_tokens: c.overlap_tokens,
            min_tokens: c.min_tokens,
        }
    }
}

impl From<BertChunkingConfigPayload> for BertChunkingConfig {
    fn from(c: BertChunkingConfigPayload) -> Self {
        Self {
            target_tokens: c.target_tokens,
            overlap_tokens: c.overlap_tokens,
            min_tokens: c.min_tokens,
        }
    }
}

impl From<SectionChunkingConfig> for SectionChunkingConfigPayload {
    fn from(c: SectionChunkingConfig) -> Self {
        Self {
            max_section_tokens: c.max_section_tokens,
        }
    }
}

impl From<SectionChunkingConfigPayload> for SectionChunkingConfig {
    fn from(c: SectionChunkingConfigPayload) -> Self {
        Self {
            max_section_tokens: c.max_section_tokens,
        }
    }
}

impl From<LlmChunkingConfig> for LlmChunkingConfigPayload {
    fn from(c: LlmChunkingConfig) -> Self {
        Self {
            target_tokens: c.target_tokens,
            micro_chunk_tokens: c.micro_chunk_tokens,
            generation_model_id: c.generation_model_id,
        }
    }
}

impl From<LlmChunkingConfigPayload> for LlmChunkingConfig {
    fn from(c: LlmChunkingConfigPayload) -> Self {
        Self {
            target_tokens: c.target_tokens,
            micro_chunk_tokens: c.micro_chunk_tokens,
            generation_model_id: c.generation_model_id,
        }
    }
}

impl From<DarnChunkingConfig> for DarnChunkingConfigPayload {
    fn from(c: DarnChunkingConfig) -> Self {
        Self {
            max_chunk_size: c.max_chunk_size,
            overlap: c.overlap,
            granularity: c.granularity.into(),
        }
    }
}

impl From<DarnChunkingConfigPayload> for DarnChunkingConfig {
    fn from(c: DarnChunkingConfigPayload) -> Self {
        Self {
            max_chunk_size: c.max_chunk_size,
            overlap: c.overlap,
            granularity: c.granularity.into(),
        }
    }
}

impl From<DarnGranularity> for DarnGranularityPayload {
    fn from(g: DarnGranularity) -> Self {
        match g {
            DarnGranularity::Characters => Self::Characters,
            DarnGranularity::Tokens => Self::Tokens,
        }
    }
}

impl From<DarnGranularityPayload> for DarnGranularity {
    fn from(g: DarnGranularityPayload) -> Self {
        match g {
            DarnGranularityPayload::Characters => Self::Characters,
            DarnGranularityPayload::Tokens => Self::Tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn section_round_trips_through_existing_wire_format() {
        let core = ChunkingConfig::Section(SectionChunkingConfig {
            max_section_tokens: 512,
        });
        let payload: ChunkingConfigPayload = core.into();
        let wire = serde_json::to_value(&payload).unwrap();
        assert_eq!(wire, json!({"section": {"max_section_tokens": 512}}));

        let decoded: ChunkingConfigPayload = serde_json::from_value(wire).unwrap();
        let back: ChunkingConfig = decoded.into();
        assert_eq!(back, core);
    }

    #[test]
    fn legacy_section_json_deserialises() {
        let legacy = json!({"section": {"max_section_tokens": 512}});
        let payload: ChunkingConfigPayload = serde_json::from_value(legacy).unwrap();
        assert!(matches!(payload, ChunkingConfigPayload::Section(_)));
    }

    #[test]
    fn legacy_darn_json_with_default_granularity() {
        let legacy = json!({"darn": {"max_chunk_size": 500, "overlap": 50}});
        let payload: ChunkingConfigPayload = serde_json::from_value(legacy).unwrap();
        if let ChunkingConfigPayload::Darn(c) = payload {
            assert_eq!(c.max_chunk_size, 500);
            assert_eq!(c.overlap, 50);
            assert_eq!(c.granularity, DarnGranularityPayload::Characters);
        } else {
            panic!("expected darn variant");
        }
    }

    #[test]
    fn legacy_string_encoded_u32_still_parses() {
        let legacy =
            json!({"bert": {"target_tokens": "384", "overlap_tokens": "64", "min_tokens": "96"}});
        let payload: ChunkingConfigPayload = serde_json::from_value(legacy).unwrap();
        if let ChunkingConfigPayload::Bert(c) = payload {
            assert_eq!(c.target_tokens, 384);
        } else {
            panic!("expected bert variant");
        }
    }

    #[test]
    fn round_trip_all_variants() {
        let cases = [
            ChunkingConfig::Bert(BertChunkingConfig::default()),
            ChunkingConfig::Section(SectionChunkingConfig::default()),
            ChunkingConfig::Llm(LlmChunkingConfig::default()),
            ChunkingConfig::Darn(DarnChunkingConfig::default()),
        ];
        for original in cases {
            let payload: ChunkingConfigPayload = original.into();
            let wire = serde_json::to_string(&payload).unwrap();
            let decoded: ChunkingConfigPayload = serde_json::from_str(&wire).unwrap();
            let restored: ChunkingConfig = decoded.into();
            assert_eq!(restored, original);
        }
    }
}
