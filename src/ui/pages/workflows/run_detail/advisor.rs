use std::cmp::Ordering;
use std::collections::HashSet;

use crate::shared::{
    evaluation_score, EvaluationResultSplit, EvaluationVariantResult, ReliabilityFlag,
};

use super::shared::{ci_overlaps, composite_ci_bounds, extract_trial_id, primary_leader, row_key};

const SMALL_SAMPLE_THRESHOLD: u32 = 25;
const WINNERS_CURSE_TRIAL_THRESHOLD: usize = 16;

#[derive(Clone)]
pub(crate) struct AdvisorEntry {
    pub flag: ReliabilityFlag,
    pub detail: String,
}

pub(crate) fn analysis_bucket_for(
    variants: &[EvaluationVariantResult],
) -> Option<EvaluationResultSplit> {
    let has_validation = variants
        .iter()
        .any(|v| v.split == EvaluationResultSplit::Validation);
    if has_validation {
        return Some(EvaluationResultSplit::Validation);
    }
    primary_leader(variants).map(|v| v.split)
}

pub(crate) fn equivalence_class_members(
    variants: &[EvaluationVariantResult],
    bucket: Option<EvaluationResultSplit>,
) -> Vec<EvaluationVariantResult> {
    let Some(bucket) = bucket else {
        return Vec::new();
    };
    let Some(bucket_leader) = variants
        .iter()
        .filter(|v| v.split == bucket)
        .max_by(|a, b| {
            evaluation_score(&a.metrics)
                .partial_cmp(&evaluation_score(&b.metrics))
                .unwrap_or(Ordering::Equal)
        })
    else {
        return Vec::new();
    };
    let (leader_lo, leader_hi) = composite_ci_bounds(bucket_leader);

    let mut out: Vec<EvaluationVariantResult> = variants
        .iter()
        .filter(|v| v.split == bucket)
        .filter(|v| {
            if row_key(v) == row_key(bucket_leader) {
                return true;
            }
            let (lo, hi) = composite_ci_bounds(v);
            leader_hi.min(hi) >= leader_lo.max(lo)
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(Ordering::Equal)
    });
    out
}

fn leader_question_count(v: &EvaluationVariantResult) -> Option<u32> {
    if v.question_results.is_empty() {
        None
    } else {
        Some(v.question_results.len() as u32)
    }
}

pub(crate) fn reliability_advisor(
    variants: &[EvaluationVariantResult],
    analysis_bucket: Option<EvaluationResultSplit>,
) -> Vec<AdvisorEntry> {
    let mut out = Vec::new();

    for holdout in variants
        .iter()
        .filter(|v| v.split == EvaluationResultSplit::Holdout)
    {
        let holdout_trial = extract_trial_id(&holdout.variant.label);
        let selection = variants
            .iter()
            .filter(|v| {
                v.split == EvaluationResultSplit::Validation
                    || v.split == EvaluationResultSplit::Tuning
            })
            .find(|v| match holdout_trial {
                Some(h_tid) => extract_trial_id(&v.variant.label) == Some(h_tid),
                None => v.variant.label == holdout.variant.label && v.options == holdout.options,
            });
        let Some(selection) = selection else {
            continue;
        };
        let h_score = evaluation_score(&holdout.metrics);
        let s_score = evaluation_score(&selection.metrics);
        let gap = (h_score - s_score).abs();
        let threshold = holdout.metrics.composite_ci_half_width().max(0.005);
        if gap > threshold {
            out.push(AdvisorEntry {
                flag: ReliabilityFlag::ValidationHoldoutGap,
                detail: format!(
                    "'{}' scored {:.1}% on {} but {:.1}% on holdout (gap {:.1}pp).",
                    holdout.variant.label,
                    s_score * 100.0,
                    selection.split.as_str(),
                    h_score * 100.0,
                    gap * 100.0,
                ),
            });
            break;
        }
    }

    if let Some(leader) = primary_leader(variants) {
        if let Some(n) = leader_question_count(leader) {
            if n > 0 && n < SMALL_SAMPLE_THRESHOLD {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::SmallSample,
                    detail: format!(
                        "Only {n} questions in the {} set; CI ±{:.1}pp on the leader.",
                        leader.split.as_str(),
                        leader.metrics.composite_ci_half_width() * 100.0,
                    ),
                });
            }
        }
    }

    if let Some(bucket) = analysis_bucket {
        let mut in_bucket: Vec<&EvaluationVariantResult> =
            variants.iter().filter(|v| v.split == bucket).collect();
        in_bucket.sort_by(|a, b| {
            evaluation_score(&b.metrics)
                .partial_cmp(&evaluation_score(&a.metrics))
                .unwrap_or(Ordering::Equal)
        });

        let q_size = in_bucket.len().div_ceil(4).clamp(2, 12);
        if let Some(&bucket_leader) = in_bucket.first().filter(|_| in_bucket.len() >= 4) {
            let top: Vec<&EvaluationVariantResult> =
                in_bucket.iter().take(q_size).copied().collect();
            if top.iter().all(|v| ci_overlaps(bucket_leader, v)) {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: format!(
                        "Top {} configs in {} overlap on CI. Dataset isn't discriminating. Consider Reasoning or Trick questions.",
                        top.len(),
                        bucket.as_str(),
                    ),
                });
            }
        }
    }

    let tuning_count = variants
        .iter()
        .filter(|v| v.split == EvaluationResultSplit::Tuning)
        .count();
    let trial_count = if tuning_count > 0 {
        let mut ids: HashSet<u32> = HashSet::new();
        for v in variants
            .iter()
            .filter(|v| v.split == EvaluationResultSplit::Tuning)
        {
            if let Some(tid) = extract_trial_id(&v.variant.label) {
                ids.insert(tid);
            }
        }
        ids.len()
    } else {
        variants.len()
    };
    if trial_count >= WINNERS_CURSE_TRIAL_THRESHOLD {
        out.push(AdvisorEntry {
            flag: ReliabilityFlag::StatisticalTie,
            detail: format!(
                "{trial_count} configs evaluated; winner's curse applies. Replicate with a new seed."
            ),
        });
    }

    if let Some(leader) = primary_leader(variants) {
        if !leader.question_results.is_empty() {
            let mut operations: HashSet<String> = HashSet::new();
            for q in &leader.question_results {
                operations.insert(q.operation.clone());
            }
            let only_recall = operations.len() == 1
                && operations.iter().next().map(String::as_str) == Some("recall");
            if only_recall {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: "Dataset is recall-only; lexical overlap inflates retrieval quality. Add comprehend / analyse / adversarial questions.".to_string(),
                });
            }
        }
    }

    out
}
