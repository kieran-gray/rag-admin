use std::cmp::Ordering;

use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::evaluation::question::EvaluationQuestion;

pub fn score_question(question: &EvaluationQuestion, retrieved: &[&Chunk]) -> (f32, f32, f32) {
    let ref_ranges = merge_ranges(&reference_ranges(question));
    let relevant_len = range_length(&ref_ranges);
    if relevant_len == 0 {
        return (0.0, 0.0, 0.0);
    }

    let retrieved_raw: Vec<(u32, u32)> = retrieved
        .iter()
        .map(|c| (c.char_start, c.char_end))
        .collect();
    let retrieved_ranges = merge_ranges(&retrieved_raw);
    let retrieved_len = range_length(&retrieved_ranges);

    let intersection_len = intersect_total(&retrieved_ranges, &ref_ranges);

    let recall = intersection_len as f32 / relevant_len as f32;
    let precision = if retrieved_len == 0 {
        0.0
    } else {
        intersection_len as f32 / retrieved_len as f32
    };
    let iou_denom = retrieved_len + relevant_len - intersection_len;
    let iou = if iou_denom == 0 {
        0.0
    } else {
        intersection_len as f32 / iou_denom as f32
    };

    (recall, precision, iou)
}

pub fn score_trick_question(retrieved_count: usize) -> (f32, f32, f32, f32) {
    if retrieved_count == 0 {
        (1.0, 1.0, 1.0, 1.0)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

pub fn precision_omega(question: &EvaluationQuestion, all_chunks: &[Chunk]) -> f32 {
    let ref_ranges = merge_ranges(&reference_ranges(question));
    if range_length(&ref_ranges) == 0 {
        return 0.0;
    }

    let mut intersection_total = 0u32;
    let mut touching_length_total = 0u32;
    for chunk in all_chunks {
        if chunk.char_end <= chunk.char_start {
            continue;
        }
        let overlap = overlap_with_ranges(chunk.char_start, chunk.char_end, &ref_ranges);
        if overlap > 0 {
            intersection_total += overlap;
            touching_length_total += chunk.char_end - chunk.char_start;
        }
    }

    if touching_length_total == 0 {
        0.0
    } else {
        intersection_total as f32 / touching_length_total as f32
    }
}

fn reference_ranges(question: &EvaluationQuestion) -> Vec<(u32, u32)> {
    question
        .references
        .iter()
        .filter(|r| r.char_end > r.char_start)
        .map(|r| (r.char_start, r.char_end))
        .collect()
}

fn merge_ranges(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut filtered: Vec<(u32, u32)> = ranges.iter().copied().filter(|&(s, e)| e > s).collect();
    if filtered.is_empty() {
        return Vec::new();
    }
    filtered.sort_by_key(|&(s, _)| s);
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(filtered.len());
    for (s, e) in filtered {
        if let Some(last) = out.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

fn range_length(ranges: &[(u32, u32)]) -> u32 {
    ranges.iter().map(|&(s, e)| e - s).sum()
}

#[expect(clippy::indexing_slicing, reason = "we know the index exists")]
fn intersect_total(a: &[(u32, u32)], b: &[(u32, u32)]) -> u32 {
    let mut total = 0u32;
    let mut i = 0;
    let mut j = 0;

    while i < a.len() && j < b.len() {
        let (a_s, a_e) = a[i];
        let (b_s, b_e) = b[j];
        let start = a_s.max(b_s);
        let end = a_e.min(b_e);
        if end > start {
            total += end - start;
        }
        if a_e <= b_e {
            i += 1;
        } else {
            j += 1;
        }
    }
    total
}

fn overlap_with_ranges(chunk_start: u32, chunk_end: u32, ranges: &[(u32, u32)]) -> u32 {
    if chunk_end <= chunk_start {
        return 0;
    }
    let mut total = 0u32;
    for &(rs, re) in ranges {
        let os = chunk_start.max(rs);
        let oe = chunk_end.min(re);
        if oe > os {
            total += oe - os;
        }
    }
    total
}

pub fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

pub fn std_dev(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

pub fn bootstrap_ci(per_question: &[f32], seed: u64, samples: usize, alpha: f32) -> (f32, f32) {
    let n = per_question.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    if let [single] = per_question {
        return (*single, *single);
    }
    let samples = samples.max(1);
    let alpha = alpha.clamp(0.0, 1.0);

    let mut state = if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    };
    let mut means: Vec<f32> = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut sum = 0.0f64;
        for _ in 0..n {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let idx = (state % n as u64) as usize;
            sum += per_question.get(idx).copied().unwrap_or(0.0) as f64;
        }
        means.push((sum / n as f64) as f32);
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

    let low = percentile(&means, alpha / 2.0);
    let high = percentile(&means, 1.0 - alpha / 2.0);
    (low, high)
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    let pos = q.clamp(0.0, 1.0) * (n - 1) as f32;
    let low = pos.floor() as usize;
    let high = pos.ceil() as usize;
    let low_v = sorted.get(low).copied().unwrap_or(0.0);
    if low == high {
        low_v
    } else {
        let frac = pos - low as f32;
        let high_v = sorted.get(high).copied().unwrap_or(low_v);
        low_v + (high_v - low_v) * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::evaluation::question::{EvaluationQuestion, EvaluationReference};
    use uuid::Uuid;

    fn chunk(start: u32, end: u32) -> Chunk {
        Chunk {
            chunk_id: Uuid::new_v4(),
            sequence: 0,
            heading: String::new(),
            text: String::new(),
            char_start: start,
            char_end: end,
        }
    }

    fn question(refs: &[(u32, u32)]) -> EvaluationQuestion {
        use crate::server::domain::evaluation::question::{
            CognitiveOperation, EvidenceKind, QuestionDimensions,
        };
        EvaluationQuestion {
            sequence: 0,
            question: "q".into(),
            references: refs
                .iter()
                .map(|&(s, e)| EvaluationReference {
                    document_id: uuid::Uuid::nil(),
                    content: String::new(),
                    char_start: s,
                    char_end: e,
                    embedding: None,
                })
                .collect(),
            embedding: None,
            dimensions: QuestionDimensions {
                operation: CognitiveOperation::Recall,
                evidence: EvidenceKind::Observation,
                role: "test".into(),
            },
            evidence_refs: Vec::new(),
        }
    }

    fn close(actual: f32, expected: f32, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-4,
            "{label}: actual={actual} expected={expected}"
        );
    }

    #[test]
    fn score_question_perfect_overlap() {
        let q = question(&[(10, 20)]);
        let retrieved = vec![chunk(10, 20)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        close(r, 1.0, "recall");
        close(p, 1.0, "precision");
        close(iou, 1.0, "iou");
    }

    #[test]
    fn score_question_partial_recall_extra_content() {
        let q = question(&[(10, 20)]);
        let retrieved = vec![chunk(0, 30)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        close(r, 1.0, "recall");
        close(p, 10.0 / 30.0, "precision");
        close(iou, 10.0 / 30.0, "iou");
    }

    #[test]
    fn score_question_no_references_returns_zero() {
        let q = question(&[]);
        let retrieved = vec![chunk(0, 10)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        assert_eq!(score_question(&q, &refs), (0.0, 0.0, 0.0));
    }

    #[test]
    fn score_trick_question_passes_only_when_nothing_retrieved() {
        assert_eq!(score_trick_question(0), (1.0, 1.0, 1.0, 1.0));
        assert_eq!(score_trick_question(1), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(score_trick_question(7), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn precision_omega_isolates_chunking_quality() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(0, 30), chunk(100, 200)];
        close(precision_omega(&q, &chunks), 10.0 / 30.0, "Pω");
    }

    #[test]
    fn merge_ranges_collapses_overlaps_and_filters_empty() {
        assert_eq!(
            merge_ranges(&[(0, 10), (5, 15), (20, 25)]),
            vec![(0, 15), (20, 25)],
        );
        assert_eq!(range_length(&[(0, 15), (20, 25)]), 20);
        assert!(merge_ranges(&[(5, 5), (7, 3)]).is_empty());
    }

    #[test]
    fn intersect_total_handles_disjoint_and_overlapping_inputs() {
        assert_eq!(intersect_total(&[(0, 10)], &[(20, 30)]), 0);
        assert_eq!(intersect_total(&[(0, 10)], &[(5, 15)]), 5);
        assert_eq!(intersect_total(&[(0, 10), (20, 30)], &[(5, 25)]), 5 + 5,);
        assert_eq!(intersect_total(&[], &[(0, 10)]), 0);
    }

    #[test]
    fn bootstrap_ci_deterministic_for_same_seed() {
        let v = vec![0.1, 0.4, 0.5, 0.7, 0.9, 0.95, 0.2, 0.6, 0.55, 0.8];
        let (l1, h1) = bootstrap_ci(&v, 42, 500, 0.05);
        let (l2, h2) = bootstrap_ci(&v, 42, 500, 0.05);
        assert!((l1 - l2).abs() < 1e-6);
        assert!((h1 - h2).abs() < 1e-6);
        assert!(l1 <= h1);
    }

    #[test]
    fn bootstrap_ci_brackets_mean_for_iid_data() {
        let v = vec![0.5; 30];
        let (l, h) = bootstrap_ci(&v, 1, 200, 0.05);
        close(l, 0.5, "ci low on constant");
        close(h, 0.5, "ci high on constant");
    }

    #[test]
    fn bootstrap_ci_empty_returns_zero() {
        assert_eq!(bootstrap_ci(&[], 1, 100, 0.05), (0.0, 0.0));
    }

    #[test]
    fn bootstrap_ci_single_value_returns_value() {
        let (l, h) = bootstrap_ci(&[0.42], 1, 100, 0.05);
        close(l, 0.42, "single low");
        close(h, 0.42, "single high");
    }

    #[test]
    fn bootstrap_ci_widens_with_variance() {
        let tight = vec![0.4, 0.5, 0.5, 0.5, 0.6];
        let wide = vec![0.0, 0.0, 0.5, 1.0, 1.0];
        let (lt, ht) = bootstrap_ci(&tight, 7, 500, 0.05);
        let (lw, hw) = bootstrap_ci(&wide, 7, 500, 0.05);
        assert!(
            (hw - lw) > (ht - lt),
            "wide CI should be wider than tight CI"
        );
    }

    #[test]
    fn bootstrap_ci_changes_with_seed() {
        let v = vec![0.1, 0.4, 0.5, 0.7, 0.9, 0.95, 0.2, 0.6, 0.55, 0.8];
        let (l1, _) = bootstrap_ci(&v, 1, 200, 0.05);
        let (l2, _) = bootstrap_ci(&v, 99, 200, 0.05);
        assert!(
            (l1 - l2).abs() > 1e-6,
            "different seeds should yield different draws"
        );
    }

    #[test]
    fn score_question_caps_recall_at_one_for_overlapping_retrieved_chunks() {
        let q = question(&[(0, 10)]);
        let retrieved = vec![chunk(0, 10), chunk(0, 10)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, _p, _iou) = score_question(&q, &refs);
        assert!(r <= 1.0 + 1e-6, "recall must not exceed 1.0, got {r}");
        close(r, 1.0, "recall capped");
    }

    #[test]
    fn score_question_handles_overlapping_retrieved_chunks_via_union() {
        let q = question(&[(2, 5)]);
        let retrieved = vec![chunk(0, 5), chunk(2, 7)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        close(r, 1.0, "recall on union");
        close(p, 3.0 / 7.0, "precision uses unique retrieved length");
        close(iou, 3.0 / 7.0, "iou uses unique retrieved length");
    }

    #[test]
    fn score_question_double_counts_neither_in_precision_nor_recall() {
        let q = question(&[(0, 10)]);
        let retrieved = vec![chunk(0, 10), chunk(0, 10), chunk(0, 10)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        close(r, 1.0, "recall");
        close(
            p,
            1.0,
            "precision should not be penalised by duplicate retrievals",
        );
        close(iou, 1.0, "iou with identical sets equals 1");
    }

    #[test]
    fn score_question_with_overlapping_references_does_not_double_count() {
        let q = question(&[(0, 10), (5, 15)]);
        let retrieved = vec![chunk(0, 20)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        close(r, 1.0, "recall after merging refs");
        close(p, 15.0 / 20.0, "precision uses merged ref length");
        close(iou, 15.0 / 20.0, "iou uses merged ref length");
    }

    #[test]
    fn score_question_empty_retrieved_returns_zero_precision_and_iou_with_full_zero_recall() {
        let q = question(&[(0, 10)]);
        let (r, p, iou) = score_question(&q, &[]);
        close(r, 0.0, "recall on empty");
        close(p, 0.0, "precision on empty");
        close(iou, 0.0, "iou on empty");
    }

    #[test]
    fn score_question_metrics_are_bounded_in_unit_interval_when_chunks_disjoint() {
        let q = question(&[(0, 100)]);
        let retrieved = vec![chunk(0, 30), chunk(50, 70), chunk(80, 90)];
        let refs: Vec<&Chunk> = retrieved.iter().collect();
        let (r, p, iou) = score_question(&q, &refs);
        for (label, v) in [("recall", r), ("precision", p), ("iou", iou)] {
            assert!(
                (0.0..=1.0 + 1e-6).contains(&v),
                "{label} must be in [0,1], got {v}",
            );
        }
    }

    #[test]
    fn precision_omega_returns_zero_when_no_chunk_touches_reference() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(0, 5), chunk(100, 200)];
        close(precision_omega(&q, &chunks), 0.0, "no touches");
    }

    #[test]
    fn precision_omega_returns_zero_when_question_has_no_references() {
        let q = question(&[]);
        let chunks = vec![chunk(0, 100)];
        close(precision_omega(&q, &chunks), 0.0, "no refs");
    }

    #[test]
    fn precision_omega_returns_one_when_chunk_exactly_matches_reference() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(10, 20)];
        close(precision_omega(&q, &chunks), 1.0, "exact match");
    }

    #[test]
    fn precision_omega_bounded_by_one_when_chunks_smaller_than_reference() {
        let q = question(&[(0, 100)]);
        let chunks = vec![chunk(50, 60)];
        let p = precision_omega(&q, &chunks);
        assert!(
            p <= 1.0 + 1e-6,
            "precision_omega must be bounded by 1, got {p}"
        );
        close(p, 1.0, "a fully relevant chunk yields p=1");
    }

    #[test]
    fn precision_omega_uses_chunk_overlap_against_merged_refs() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(0, 30), chunk(40, 80)];
        let p = precision_omega(&q, &chunks);
        close(
            p,
            10.0 / 30.0,
            "only chunk (0,30) touches; overlap=10, chunk_len=30",
        );
    }

    #[test]
    fn precision_omega_averages_across_multiple_touching_chunks() {
        let q = question(&[(0, 100)]);
        let chunks = vec![chunk(0, 20), chunk(40, 60), chunk(80, 100)];
        let p = precision_omega(&q, &chunks);
        close(p, 1.0, "every touching chunk is fully relevant");
    }

    #[test]
    fn precision_omega_with_partial_overlap_uses_overlap_in_numerator() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(0, 50)];
        let p = precision_omega(&q, &chunks);
        close(p, 10.0 / 50.0, "overlap=10, chunk_len=50");
    }

    #[test]
    fn mean_of_empty_is_zero() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn mean_matches_arithmetic_mean() {
        close(mean(&[1.0, 2.0, 3.0, 4.0]), 2.5, "mean");
    }

    #[test]
    fn std_dev_of_single_value_is_zero() {
        assert_eq!(std_dev(&[0.5]), 0.0);
    }

    #[test]
    fn std_dev_of_constant_values_is_zero() {
        close(std_dev(&[0.7, 0.7, 0.7, 0.7]), 0.0, "constant std_dev");
    }

    #[test]
    fn std_dev_uses_population_formula_dividing_by_n() {
        let v = [0.0_f32, 4.0];
        close(std_dev(&v), 2.0, "population std_dev");
    }

    #[test]
    fn bootstrap_ci_low_le_high_invariant() {
        let v = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        for seed in [1u64, 2, 7, 42, 99] {
            let (lo, hi) = bootstrap_ci(&v, seed, 100, 0.05);
            assert!(lo <= hi, "seed {seed}: low={lo} hi={hi}");
        }
    }

    #[test]
    fn bootstrap_ci_alpha_one_collapses_around_median() {
        let v: Vec<f32> = (0..30).map(|i| (i as f32) / 30.0).collect();
        let (lo, hi) = bootstrap_ci(&v, 5, 200, 1.0);
        assert!((lo - hi).abs() < 0.1, "alpha=1 should collapse to a point");
    }
}
