use leptos::prelude::*;

use crate::core::{
    evaluation_score, EvaluationResultSplit, EvaluationVariantResult, ReliabilityFlag,
};
use crate::ui::components::primitives::Surface;

use super::shared::{ci_overlaps, composite_ci_bounds, extract_trial_id, primary_leader, row_key};

const SMALL_SAMPLE_THRESHOLD: u32 = 25;
const WINNERS_CURSE_TRIAL_THRESHOLD: usize = 16;

#[derive(Clone)]
pub(super) struct AdvisorEntry {
    pub flag: ReliabilityFlag,
    pub detail: String,
}

pub(super) fn analysis_bucket_for(
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

pub(super) fn equivalence_class_members(
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
                .unwrap_or(std::cmp::Ordering::Equal)
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
            .unwrap_or(std::cmp::Ordering::Equal)
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

pub(super) fn reliability_advisor(
    variants: &[EvaluationVariantResult],
    tied: &[EvaluationVariantResult],
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
                    "'{}' scored {:.1}% on {} but {:.1}% on holdout (gap {:.1}pp, holdout 95% confidence interval half-width {:.1}pp). The selection may have overfit.",
                    holdout.variant.label,
                    s_score * 100.0,
                    selection.split.as_str(),
                    h_score * 100.0,
                    gap * 100.0,
                    threshold * 100.0,
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
                        "Only {n} questions in the {} set. The 95% confidence interval is ±{:.1}pp on the leader; scores within that band are not distinguishable.",
                        leader.split.as_str(),
                        leader.metrics.composite_ci_half_width() * 100.0,
                    ),
                });
            }
        }
    }

    if tied.len() > 1 {
        let bucket_label = analysis_bucket
            .map(|b| b.as_str())
            .unwrap_or("the analysis bucket");
        out.push(AdvisorEntry {
            flag: ReliabilityFlag::StatisticalTie,
            detail: format!(
                "{} configs have 95% confidence intervals that overlap the {bucket_label} leader's. Confidence-interval overlap is a heuristic, not a formal equivalence test, but the ranking between them is unlikely to be reliable. Prefer the cheapest or simplest.",
                tied.len(),
            ),
        });
    }

    if let Some(bucket) = analysis_bucket {
        let mut in_bucket: Vec<&EvaluationVariantResult> =
            variants.iter().filter(|v| v.split == bucket).collect();
        in_bucket.sort_by(|a, b| {
            evaluation_score(&b.metrics)
                .partial_cmp(&evaluation_score(&a.metrics))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let q_size = in_bucket.len().div_ceil(4).clamp(2, 12);
        if let Some(&bucket_leader) = in_bucket.first().filter(|_| in_bucket.len() >= 4) {
            let top: Vec<&EvaluationVariantResult> =
                in_bucket.iter().take(q_size).copied().collect();
            if top.iter().all(|v| ci_overlaps(bucket_leader, v)) {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: format!(
                        "The top {} configs in the {} bucket all overlap in their 95% confidence intervals. The dataset isn't discriminating between them. Consider adding Reasoning or Trick category questions.",
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
        let mut ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
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
                "{trial_count} configs were evaluated. The max-over-N estimator is biased upward (winner's curse), and 95% confidence intervals aren't multiple-comparison adjusted; the reported best score is more optimistic than a single fresh run on the same config would produce. Replicate with a new seed to sanity-check."
            ),
        });
    }

    if let Some(leader) = primary_leader(variants) {
        if !leader.question_results.is_empty() {
            let mut categories: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for q in &leader.question_results {
                categories.insert(q.category.clone());
            }
            let only_fact = categories.len() == 1
                && categories.iter().next().map(String::as_str) == Some("fact_retrieval");
            if only_fact {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: "Synthetic dataset is fact-retrieval only. Generator-shaped, extractive benchmarks reward lexical overlap; results will overstate retrieval quality on broader user queries. Add Architecture, Reasoning, or Trick category questions for a more honest signal.".to_string(),
                });
            }
        }
    }

    out
}

#[component]
pub(super) fn ReliabilityAdvisor(entries: Vec<AdvisorEntry>) -> impl IntoView {
    view! {
        <Surface title="Reliability advisor".to_string()>
            <div class="space-y-3">
                {entries.into_iter().map(|e| view! {
                    <div class="advisor-banner">
                        <div class="advisor-banner-headline">{e.flag.headline()}</div>
                        <div class="advisor-banner-detail muted">{e.detail}</div>
                    </div>
                }).collect_view()}
            </div>
        </Surface>
    }
}
