use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::{
    BertChunkingConfig as BertChunkingConfigDto, ChunkingConfig as ChunkingConfigDto,
    DarnChunkingConfig as DarnChunkingConfigDto, DarnGranularity as DarnGranularityDto,
    LlmChunkingConfig as LlmChunkingConfigDto, SectionChunkingConfig as SectionChunkingConfigDto,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingConfig {
    Bert(BertChunkingConfig),
    Section(SectionChunkingConfig),
    Llm(LlmChunkingConfig),
    Darn(DarnChunkingConfig),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionChunkingConfig {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub max_section_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmChunkingConfig {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub target_tokens: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub micro_chunk_tokens: u32,
    pub generation_model_id: Uuid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DarnChunkingConfig {
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub max_chunk_size: u32,
    #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
    pub overlap: u32,
    #[serde(default)]
    pub granularity: DarnGranularity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DarnGranularity {
    #[default]
    Characters,
    Tokens,
}

impl From<ChunkingConfigDto> for ChunkingConfig {
    fn from(value: ChunkingConfigDto) -> Self {
        match value {
            ChunkingConfigDto::Bert(c) => Self::Bert(c.into()),
            ChunkingConfigDto::Section(c) => Self::Section(c.into()),
            ChunkingConfigDto::Llm(c) => Self::Llm(c.into()),
            ChunkingConfigDto::Darn(c) => Self::Darn(c.into()),
        }
    }
}

impl From<ChunkingConfig> for ChunkingConfigDto {
    fn from(value: ChunkingConfig) -> Self {
        match value {
            ChunkingConfig::Bert(c) => Self::Bert(c.into()),
            ChunkingConfig::Section(c) => Self::Section(c.into()),
            ChunkingConfig::Llm(c) => Self::Llm(c.into()),
            ChunkingConfig::Darn(c) => Self::Darn(c.into()),
        }
    }
}

impl From<BertChunkingConfigDto> for BertChunkingConfig {
    fn from(c: BertChunkingConfigDto) -> Self {
        Self {
            target_tokens: c.target_tokens,
            overlap_tokens: c.overlap_tokens,
            min_tokens: c.min_tokens,
        }
    }
}

impl From<BertChunkingConfig> for BertChunkingConfigDto {
    fn from(c: BertChunkingConfig) -> Self {
        Self {
            target_tokens: c.target_tokens,
            overlap_tokens: c.overlap_tokens,
            min_tokens: c.min_tokens,
        }
    }
}

impl From<SectionChunkingConfigDto> for SectionChunkingConfig {
    fn from(c: SectionChunkingConfigDto) -> Self {
        Self {
            max_section_tokens: c.max_section_tokens,
        }
    }
}

impl From<SectionChunkingConfig> for SectionChunkingConfigDto {
    fn from(c: SectionChunkingConfig) -> Self {
        Self {
            max_section_tokens: c.max_section_tokens,
        }
    }
}

impl From<LlmChunkingConfigDto> for LlmChunkingConfig {
    fn from(c: LlmChunkingConfigDto) -> Self {
        Self {
            target_tokens: c.target_tokens,
            micro_chunk_tokens: c.micro_chunk_tokens,
            generation_model_id: c.generation_model_id,
        }
    }
}

impl From<LlmChunkingConfig> for LlmChunkingConfigDto {
    fn from(c: LlmChunkingConfig) -> Self {
        Self {
            target_tokens: c.target_tokens,
            micro_chunk_tokens: c.micro_chunk_tokens,
            generation_model_id: c.generation_model_id,
        }
    }
}

impl From<DarnChunkingConfigDto> for DarnChunkingConfig {
    fn from(c: DarnChunkingConfigDto) -> Self {
        Self {
            max_chunk_size: c.max_chunk_size,
            overlap: c.overlap,
            granularity: c.granularity.into(),
        }
    }
}

impl From<DarnChunkingConfig> for DarnChunkingConfigDto {
    fn from(c: DarnChunkingConfig) -> Self {
        Self {
            max_chunk_size: c.max_chunk_size,
            overlap: c.overlap,
            granularity: c.granularity.into(),
        }
    }
}

impl From<DarnGranularityDto> for DarnGranularity {
    fn from(g: DarnGranularityDto) -> Self {
        match g {
            DarnGranularityDto::Characters => Self::Characters,
            DarnGranularityDto::Tokens => Self::Tokens,
        }
    }
}

impl From<DarnGranularity> for DarnGranularityDto {
    fn from(g: DarnGranularity) -> Self {
        match g {
            DarnGranularity::Characters => Self::Characters,
            DarnGranularity::Tokens => Self::Tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn section_round_trips_through_existing_wire_format() {
        let core = ChunkingConfigDto::Section(SectionChunkingConfigDto {
            max_section_tokens: 512,
        });
        let domain: ChunkingConfig = core.into();
        let wire = serde_json::to_value(&domain).unwrap();
        assert_eq!(wire, json!({"section": {"max_section_tokens": 512}}));

        let decoded: ChunkingConfig = serde_json::from_value(wire).unwrap();
        let back: ChunkingConfigDto = decoded.into();
        assert_eq!(back, core);
    }

    #[test]
    fn legacy_section_json_deserialises() {
        let legacy = json!({"section": {"max_section_tokens": 512}});
        let domain: ChunkingConfig = serde_json::from_value(legacy).unwrap();
        assert!(matches!(domain, ChunkingConfig::Section(_)));
    }

    #[test]
    fn legacy_darn_json_with_default_granularity() {
        let legacy = json!({"darn": {"max_chunk_size": 500, "overlap": 50}});
        let domain: ChunkingConfig = serde_json::from_value(legacy).unwrap();
        if let ChunkingConfig::Darn(c) = domain {
            assert_eq!(c.max_chunk_size, 500);
            assert_eq!(c.overlap, 50);
            assert_eq!(c.granularity, DarnGranularity::Characters);
        } else {
            panic!("expected darn variant");
        }
    }

    #[test]
    fn legacy_string_encoded_u32_still_parses() {
        let legacy =
            json!({"bert": {"target_tokens": "384", "overlap_tokens": "64", "min_tokens": "96"}});
        let domain: ChunkingConfig = serde_json::from_value(legacy).unwrap();
        if let ChunkingConfig::Bert(c) = domain {
            assert_eq!(c.target_tokens, 384);
        } else {
            panic!("expected bert variant");
        }
    }

    #[test]
    fn round_trip_all_variants() {
        let cases = [
            ChunkingConfigDto::Bert(BertChunkingConfigDto::default()),
            ChunkingConfigDto::Section(SectionChunkingConfigDto::default()),
            ChunkingConfigDto::Llm(LlmChunkingConfigDto::default()),
            ChunkingConfigDto::Darn(DarnChunkingConfigDto::default()),
        ];
        for original in cases {
            let domain: ChunkingConfig = original.into();
            let wire = serde_json::to_string(&domain).unwrap();
            let decoded: ChunkingConfig = serde_json::from_str(&wire).unwrap();
            let restored: ChunkingConfigDto = decoded.into();
            assert_eq!(restored, original);
        }
    }
}
