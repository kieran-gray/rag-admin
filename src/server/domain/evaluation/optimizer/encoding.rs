use std::collections::HashMap;

use serde_json::{json, Map};
use uuid::Uuid;

use crate::core::{
    BertChunkingConfig, ChunkingConfig, DarnChunkingConfig, DarnGranularity, EvaluationRunOptions,
    LlmChunkingConfig, SectionChunkingConfig,
};

use super::search_space::Value;

pub fn params_to_json(params: &HashMap<String, Value>) -> serde_json::Value {
    let mut obj = Map::with_capacity(params.len());
    for (k, v) in params {
        let json_v = match v {
            Value::String(s) => json!(s),
            Value::Int(n) => json!(n),
            Value::Float(f) => json!(f),
        };
        obj.insert(k.clone(), json_v);
    }
    serde_json::Value::Object(obj)
}

pub fn params_from_json(value: &serde_json::Value) -> HashMap<String, Value> {
    let mut out: HashMap<String, Value> = HashMap::new();
    let Some(obj) = value.as_object() else {
        return out;
    };
    for (k, v) in obj {
        let value = match v {
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        out.insert(k.clone(), value);
    }
    out
}

pub fn params_to_run_config(
    params: &HashMap<String, Value>,
    generation_model_id: Uuid,
) -> (ChunkingConfig, EvaluationRunOptions) {
    let strategy = params
        .get("strategy")
        .and_then(|v| v.as_string())
        .unwrap_or("section");

    let chunking = match strategy {
        "bert" => {
            let target_tokens = params
                .get("bert_target_tokens")
                .and_then(|v| v.as_int())
                .map(|n| n.clamp(1, 8192) as u32)
                .unwrap_or_else(|| BertChunkingConfig::default().target_tokens);
            let defaults = BertChunkingConfig::default();
            ChunkingConfig::Bert(BertChunkingConfig {
                target_tokens,
                overlap_tokens: defaults.overlap_tokens,
                min_tokens: defaults.min_tokens,
            })
        }
        "llm" => {
            let defaults = LlmChunkingConfig::default();
            let micro_chunk_tokens = params
                .get("micro_chunk_tokens")
                .and_then(|v| v.as_int())
                .map(|n| n.clamp(1, 8192) as u32)
                .unwrap_or(defaults.micro_chunk_tokens);
            ChunkingConfig::Llm(LlmChunkingConfig {
                target_tokens: defaults.target_tokens,
                micro_chunk_tokens,
                generation_model_id,
            })
        }
        "darn" => {
            let defaults = DarnChunkingConfig::default();
            let max_chunk_size = params
                .get("darn_max_chunk_size")
                .and_then(|v| v.as_int())
                .map(|n| n.clamp(1, 8192) as u32)
                .unwrap_or(defaults.max_chunk_size);
            let overlap = params
                .get("darn_overlap")
                .and_then(|v| v.as_int())
                .map(|n| n.clamp(0, 8192) as u32)
                .unwrap_or(defaults.overlap);
            ChunkingConfig::Darn(DarnChunkingConfig {
                max_chunk_size,
                overlap,
                granularity: DarnGranularity::Characters,
            })
        }
        _ => {
            let max_section_tokens = params
                .get("max_section_tokens")
                .and_then(|v| v.as_int())
                .map(|n| n.clamp(1, 8192) as u32)
                .unwrap_or_else(|| SectionChunkingConfig::default().max_section_tokens);
            ChunkingConfig::Section(SectionChunkingConfig { max_section_tokens })
        }
    };

    let top_k = params
        .get("top_k")
        .and_then(|v| v.as_int())
        .map(|n| n.clamp(1, 1_000) as u32)
        .unwrap_or_else(|| EvaluationRunOptions::default().top_k);
    let min_score_milli = params
        .get("min_score")
        .and_then(|v| v.as_float())
        .map(|f| (f.clamp(0.0, 1.0) * 1000.0).round() as u32)
        .unwrap_or(0);
    let options = EvaluationRunOptions {
        top_k,
        min_score_milli,
    };

    (chunking, options)
}

pub fn trial_label(trial_id: u32) -> String {
    format!("trial-{:04}", trial_id)
}

pub fn trial_rung_label(trial_id: u32, rung: u32) -> String {
    format!("trial-{:04}-r{}", trial_id, rung)
}

pub fn trial_validation_label(trial_id: u32) -> String {
    format!("trial-{:04}-validation", trial_id)
}

pub fn trial_holdout_label(trial_id: u32) -> String {
    format!("trial-{:04}-holdout", trial_id)
}

pub fn parse_trial_rung_label(label: &str) -> Option<(u32, u32)> {
    let rest = label.strip_prefix("trial-")?;
    let (trial_str, rung_str) = rest.split_once("-r")?;
    let trial_id: u32 = trial_str.parse().ok()?;
    let rung: u32 = rung_str.parse().ok()?;
    Some((trial_id, rung))
}

pub fn parse_trial_validation_label(label: &str) -> Option<u32> {
    let rest = label.strip_prefix("trial-")?;
    let trial_str = rest.strip_suffix("-validation")?;
    trial_str.parse().ok()
}

pub fn parse_trial_holdout_label(label: &str) -> Option<u32> {
    let rest = label.strip_prefix("trial-")?;
    let trial_str = rest.strip_suffix("-holdout")?;
    trial_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_preserves_value_kinds() {
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("strategy".into(), Value::String("section".into()));
        params.insert("max_section_tokens".into(), Value::Int(512));
        params.insert("min_score".into(), Value::Float(0.42));
        params.insert("top_k".into(), Value::Int(7));

        let json = params_to_json(&params);
        let round_tripped = params_from_json(&json);
        assert_eq!(round_tripped.len(), 4);
        assert!(matches!(
            round_tripped.get("strategy"),
            Some(Value::String(s)) if s == "section"
        ));
        assert!(matches!(
            round_tripped.get("max_section_tokens"),
            Some(Value::Int(512))
        ));
        assert!(matches!(round_tripped.get("top_k"), Some(Value::Int(7))));
        let Some(Value::Float(f)) = round_tripped.get("min_score") else {
            panic!("min_score should round-trip as float");
        };
        assert!((f - 0.42).abs() < 1e-9);
    }

    #[test]
    fn params_from_json_ignores_non_object() {
        let v = json!([1, 2, 3]);
        assert!(params_from_json(&v).is_empty());
    }

    #[test]
    fn section_strategy_maps_to_section_chunking_with_token_override() {
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("strategy".into(), Value::String("section".into()));
        params.insert("max_section_tokens".into(), Value::Int(640));
        params.insert("top_k".into(), Value::Int(5));
        params.insert("min_score".into(), Value::Float(0.1));

        let (chunking, options) = params_to_run_config(&params, Uuid::nil());
        assert!(
            matches!(chunking, ChunkingConfig::Section(c) if c.max_section_tokens == 640),
            "section override should be honoured",
        );
        assert_eq!(options.top_k, 5);
        assert_eq!(options.min_score_milli, 100);
    }

    #[test]
    fn bert_strategy_reads_its_own_gated_token_param() {
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("strategy".into(), Value::String("bert".into()));
        params.insert("bert_target_tokens".into(), Value::Int(256));

        params.insert("max_section_tokens".into(), Value::Int(9999));
        params.insert("top_k".into(), Value::Int(10));
        params.insert("min_score".into(), Value::Float(0.0));

        let (chunking, _options) = params_to_run_config(&params, Uuid::nil());
        assert!(matches!(
            chunking,
            ChunkingConfig::Bert(c) if c.target_tokens == 256
        ));
    }

    #[test]
    fn llm_strategy_carries_pipeline_generation_model_and_micro_tokens() {
        let model_id = Uuid::new_v4();
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("strategy".into(), Value::String("llm".into()));
        params.insert("micro_chunk_tokens".into(), Value::Int(128));

        let (chunking, _options) = params_to_run_config(&params, model_id);
        let ChunkingConfig::Llm(c) = chunking else {
            panic!("expected llm chunking config");
        };
        assert_eq!(c.generation_model_id, model_id);
        assert_eq!(c.micro_chunk_tokens, 128);
    }

    #[test]
    fn missing_params_fall_back_to_type_defaults() {
        let params: HashMap<String, Value> = HashMap::new();
        let (chunking, options) = params_to_run_config(&params, Uuid::nil());
        assert!(matches!(chunking, ChunkingConfig::Section(_)));
        assert_eq!(options.top_k, EvaluationRunOptions::default().top_k);
        assert_eq!(options.min_score_milli, 0);
    }

    #[test]
    fn min_score_clamps_to_unit_range_and_rounds_to_milli() {
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("min_score".into(), Value::Float(0.7351));
        let (_chunking, options) = params_to_run_config(&params, Uuid::nil());
        assert_eq!(options.min_score_milli, 735);

        let mut clamped: HashMap<String, Value> = HashMap::new();
        clamped.insert("min_score".into(), Value::Float(1.5));
        let (_c, options) = params_to_run_config(&clamped, Uuid::nil());
        assert_eq!(options.min_score_milli, 1000);
    }

    #[test]
    fn trial_label_is_zero_padded() {
        assert_eq!(trial_label(0), "trial-0000");
        assert_eq!(trial_label(42), "trial-0042");
        assert_eq!(trial_label(9999), "trial-9999");
    }
}
