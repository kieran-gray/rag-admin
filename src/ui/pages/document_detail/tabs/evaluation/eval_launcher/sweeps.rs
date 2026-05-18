use uuid::Uuid;

use crate::shared::contracts::{ChunkingConfigurationDto, SweepTemplateDto};
use crate::shared::reference_data::ChunkStrategy;
use crate::shared::{
    BertChunkingConfig, ChunkingConfig, ChunkingVariant, DarnChunkingConfig, DarnGranularity,
    LlmChunkingConfig, SectionChunkingConfig,
};

#[allow(dead_code)]
pub(super) const SWEEP_TEMPLATE_STORAGE_KEY: &str = "eval_launcher.sweep_template_id";

#[allow(clippy::too_many_arguments)]
pub(super) fn build_single_variant(
    strategy: ChunkStrategy,
    section: u32,
    bert_target: u32,
    bert_overlap: u32,
    llm_micro: u32,
    darn_size: u32,
    darn_overlap: u32,
    generation_model_id: Uuid,
) -> Result<ChunkingVariant, String> {
    Ok(match strategy {
        ChunkStrategy::Section => ChunkingVariant {
            label: format!("section:{section}"),
            config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: section,
            }),
        },
        ChunkStrategy::Bert => ChunkingVariant {
            label: format!("bert:{bert_target}/{bert_overlap}"),
            config: ChunkingConfig::Bert(BertChunkingConfig {
                target_tokens: bert_target,
                overlap_tokens: bert_overlap,
                min_tokens: 96,
            }),
        },
        ChunkStrategy::Llm => ChunkingVariant {
            label: format!("llm:{llm_micro}"),
            config: ChunkingConfig::Llm(LlmChunkingConfig {
                target_tokens: 384,
                micro_chunk_tokens: llm_micro,
                generation_model_id,
            }),
        },
        ChunkStrategy::Darn => ChunkingVariant {
            label: format!("darn:{darn_size}/{darn_overlap}"),
            config: ChunkingConfig::Darn(DarnChunkingConfig {
                max_chunk_size: darn_size,
                overlap: darn_overlap,
                granularity: DarnGranularity::Characters,
            }),
        },
    })
}

pub(super) fn default_sweep_variants(seeds: &[ChunkingConfigurationDto]) -> Vec<ChunkingVariant> {
    seeds
        .iter()
        .map(|cc| ChunkingVariant {
            label: cc.name.clone(),
            config: cc.config,
        })
        .collect()
}

pub(super) fn template_variants(
    template: &SweepTemplateDto,
    configs: &[ChunkingConfigurationDto],
) -> Vec<ChunkingVariant> {
    template
        .members
        .iter()
        .filter_map(|id| {
            configs
                .iter()
                .find(|cc| cc.chunking_configuration_id == *id)
                .map(|cc| ChunkingVariant {
                    label: cc.name.clone(),
                    config: cc.config,
                })
        })
        .collect()
}

pub(super) fn build_section_sweep(values: Vec<u32>) -> Vec<ChunkingVariant> {
    values
        .into_iter()
        .map(|t| ChunkingVariant {
            label: format!("section:{t}"),
            config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: t,
            }),
        })
        .collect()
}

pub(super) fn build_bert_sweep(targets: &[u32], overlaps: &[u32]) -> Vec<ChunkingVariant> {
    let mut out = Vec::with_capacity(targets.len() * overlaps.len());
    for &t in targets {
        for &o in overlaps {
            out.push(ChunkingVariant {
                label: format!("bert:{t}/{o}"),
                config: ChunkingConfig::Bert(BertChunkingConfig {
                    target_tokens: t,
                    overlap_tokens: o,
                    min_tokens: 96,
                }),
            });
        }
    }
    out
}

pub(super) fn build_llm_sweep(values: Vec<u32>, generation_model_id: Uuid) -> Vec<ChunkingVariant> {
    values
        .into_iter()
        .map(|micro| ChunkingVariant {
            label: format!("llm:{micro}"),
            config: ChunkingConfig::Llm(LlmChunkingConfig {
                target_tokens: 384,
                micro_chunk_tokens: micro,
                generation_model_id,
            }),
        })
        .collect()
}

pub(super) fn build_darn_sweep(values: Vec<u32>, overlap: u32) -> Vec<ChunkingVariant> {
    values
        .into_iter()
        .map(|size| ChunkingVariant {
            label: format!("darn:{size}/{overlap}"),
            config: ChunkingConfig::Darn(DarnChunkingConfig {
                max_chunk_size: size,
                overlap,
                granularity: DarnGranularity::Characters,
            }),
        })
        .collect()
}

#[cfg(feature = "hydrate")]
pub(super) fn load_sweep_template_pref() -> Option<Uuid> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let raw = storage
        .get_item(SWEEP_TEMPLATE_STORAGE_KEY)
        .ok()
        .flatten()?;
    Uuid::parse_str(&raw).ok()
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn load_sweep_template_pref() -> Option<Uuid> {
    None
}

#[cfg(feature = "hydrate")]
pub(super) fn store_sweep_template_pref(id: Option<Uuid>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(storage) = window.local_storage().ok().flatten() else {
        return;
    };
    match id {
        Some(id) => {
            _ = storage.set_item(SWEEP_TEMPLATE_STORAGE_KEY, &id.to_string());
        }
        None => {
            _ = storage.remove_item(SWEEP_TEMPLATE_STORAGE_KEY);
        }
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn store_sweep_template_pref(_: Option<Uuid>) {}
