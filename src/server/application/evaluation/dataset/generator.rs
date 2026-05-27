use std::collections::HashMap;

use crate::server::domain::evaluation::dataset::events::AxisWeight;
use crate::server::domain::evaluation::question::{
    is_combination_valid, CognitiveOperation, EvidenceKind,
};

#[derive(Debug, Clone)]
pub struct GenerationPlan {
    pub by_axis: HashMap<(CognitiveOperation, EvidenceKind), u32>,
}

impl GenerationPlan {
    pub fn from_matrix(target: u32, weight_matrix: &[AxisWeight]) -> Self {
        let total: u64 = weight_matrix.iter().map(|w| w.weight as u64).sum();
        let mut by_axis = HashMap::new();
        if total == 0 {
            return Self { by_axis };
        }
        for w in weight_matrix {
            let Ok(op) = CognitiveOperation::parse(&w.operation) else {
                continue;
            };
            let Ok(ev) = EvidenceKind::parse(&w.evidence) else {
                continue;
            };
            if !is_combination_valid(op, ev) {
                continue;
            }
            let n = (target as u64 * w.weight as u64 / total) as u32;
            by_axis.insert((op, ev), n);
        }
        Self { by_axis }
    }

    pub fn default_weight_matrix() -> Vec<AxisWeight> {
        let defaults: &[(CognitiveOperation, EvidenceKind, u32)] = &[
            (CognitiveOperation::Recall, EvidenceKind::Observation, 25),
            (
                CognitiveOperation::Comprehend,
                EvidenceKind::Observation,
                15,
            ),
            (CognitiveOperation::Comprehend, EvidenceKind::Thread, 10),
            (CognitiveOperation::Analyse, EvidenceKind::Thread, 10),
            (CognitiveOperation::Apply, EvidenceKind::Thread, 10),
            (CognitiveOperation::Analyse, EvidenceKind::Insight, 5),
            (CognitiveOperation::Synthesise, EvidenceKind::Insight, 10),
            (CognitiveOperation::Evaluate, EvidenceKind::Insight, 5),
            (
                CognitiveOperation::Adversarial,
                EvidenceKind::Observation,
                10,
            ),
        ];
        defaults
            .iter()
            .map(|(op, ev, w)| AxisWeight {
                operation: op.as_str().into(),
                evidence: ev.as_str().into(),
                weight: *w,
            })
            .collect()
    }
}
