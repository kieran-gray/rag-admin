use std::cmp::Ordering;

use crate::shared::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};

#[derive(Clone, Copy)]
pub(crate) struct MetricDef {
    pub name: &'static str,
    pub short: &'static str,
    pub help: &'static str,
}

pub(crate) const RECALL_DEF: MetricDef = MetricDef {
    name: "Recall",
    short: "R",
    help: "Fraction of each question's reference span that the retrieved chunks cover. \
           1.0 means every byte of every reference was returned. Penalised by missing content.",
};

pub(crate) const PRECISION_DEF: MetricDef = MetricDef {
    name: "Precision",
    short: "P",
    help: "Fraction of the retrieved chunks' bytes that fall inside a reference span. \
           Penalised by retrieving extra non-relevant content alongside the answer.",
};

pub(crate) const IOU_DEF: MetricDef = MetricDef {
    name: "IoU",
    short: "IoU",
    help: "Intersection-over-Union: overlap between retrieved and reference spans divided \
           by their union. A combined measure that punishes both missed and excess content.",
};

pub(crate) const PRECISION_OMEGA_DEF: MetricDef = MetricDef {
    name: "Precision-ω",
    short: "Pω",
    help: "Precision over only the chunks whose spans touch a reference. \
           Isolates retrieval quality from chunk-boundary noise.",
};

pub(crate) const METRIC_DEFS: &[MetricDef] =
    &[RECALL_DEF, PRECISION_DEF, IOU_DEF, PRECISION_OMEGA_DEF];

#[derive(Clone, Copy, Default)]
pub(crate) struct MetricBests {
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
    pub precision_omega: f32,
}

pub(crate) fn best_per_metric(variants: &[EvaluationVariantResult]) -> MetricBests {
    let mut b = MetricBests::default();
    for v in variants {
        b.recall = b.recall.max(v.metrics.recall_mean);
        b.precision = b.precision.max(v.metrics.precision_mean);
        b.iou = b.iou.max(v.metrics.iou_mean);
        b.precision_omega = b.precision_omega.max(v.metrics.precision_omega_mean);
    }
    b
}

pub(crate) fn variant_display(v: &EvaluationVariantResult) -> (String, Option<String>) {
    let config_label = v.variant.config.display_label();
    if v.variant.label == config_label {
        (config_label, None)
    } else {
        (config_label, Some(v.variant.label.clone()))
    }
}

pub(crate) fn row_key(v: &EvaluationVariantResult) -> String {
    format!(
        "{}|{}|{}|{}",
        v.variant.label,
        v.split.as_str(),
        v.options.top_k,
        v.options.min_score_milli
    )
}

pub(crate) fn primary_leader(
    variants: &[EvaluationVariantResult],
) -> Option<&EvaluationVariantResult> {
    const PRIORITY: [EvaluationResultSplit; 4] = [
        EvaluationResultSplit::Holdout,
        EvaluationResultSplit::Validation,
        EvaluationResultSplit::Full,
        EvaluationResultSplit::Tuning,
    ];
    let bucket = PRIORITY
        .iter()
        .copied()
        .find(|b| variants.iter().any(|v| v.split == *b))?;
    variants
        .iter()
        .filter(|v| v.split == bucket)
        .max_by(|a, b| {
            evaluation_score(&a.metrics)
                .partial_cmp(&evaluation_score(&b.metrics))
                .unwrap_or(Ordering::Equal)
        })
}

pub(crate) fn extract_trial_id(label: &str) -> Option<u32> {
    let rest = label.strip_prefix("trial-")?;
    let head = rest.split('-').next().unwrap_or(rest);
    head.parse().ok()
}

pub(crate) fn composite_ci_bounds(variant: &EvaluationVariantResult) -> (f32, f32) {
    let low = variant.metrics.composite_ci_low;
    let high = variant.metrics.composite_ci_high;
    if high > low {
        (low, high)
    } else {
        let s = evaluation_score(&variant.metrics);
        (s, s)
    }
}

pub(crate) fn ci_overlaps(a: &EvaluationVariantResult, b: &EvaluationVariantResult) -> bool {
    let (a_lo, a_hi) = composite_ci_bounds(a);
    let (b_lo, b_hi) = composite_ci_bounds(b);
    a_hi.min(b_hi) >= a_lo.max(b_lo)
}

pub(crate) fn ci_half(low: f32, high: f32) -> f32 {
    ((high - low) / 2.0).max(0.0)
}
