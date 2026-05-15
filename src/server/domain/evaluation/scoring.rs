use crate::server::domain::chunk_set::entity::Chunk;
use crate::server::domain::evaluation::question::EvaluationQuestion;

pub fn score_question(question: &EvaluationQuestion, retrieved: &[&Chunk]) -> (f32, f32, f32) {
    let reference_ranges = reference_ranges(question);
    let relevant_len = non_overlapping_len(&reference_ranges);
    if relevant_len == 0 {
        return (0.0, 0.0, 0.0);
    }

    let mut intersection_len = 0u32;
    for chunk in retrieved {
        for &(ref_start, ref_end) in &reference_ranges {
            let overlap_start = chunk.char_start.max(ref_start);
            let overlap_end = chunk.char_end.min(ref_end);
            if overlap_end > overlap_start {
                intersection_len += overlap_end - overlap_start;
            }
        }
    }
    let intersection_len = intersection_len.min(relevant_len);

    let retrieved_len: u32 = retrieved.iter().map(|c| c.char_end - c.char_start).sum();
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
    let reference_ranges = reference_ranges(question);
    let relevant_len = non_overlapping_len(&reference_ranges);
    if relevant_len == 0 {
        return 0.0;
    }

    let min_possible: u32 = all_chunks
        .iter()
        .map(|c| {
            let touches_reference = reference_ranges.iter().any(|&(rs, re)| {
                let os = c.char_start.max(rs);
                let oe = c.char_end.min(re);
                oe > os
            });
            if touches_reference {
                c.char_end - c.char_start
            } else {
                0
            }
        })
        .sum();

    if min_possible == 0 {
        0.0
    } else {
        relevant_len as f32 / min_possible as f32
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

fn non_overlapping_len(ranges: &[(u32, u32)]) -> u32 {
    if ranges.is_empty() {
        return 0;
    }
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|&(s, _)| s);
    let mut total = 0u32;
    let mut cur_end = 0u32;
    for (s, e) in sorted {
        if s >= cur_end {
            total += e - s;
            cur_end = e;
        } else if e > cur_end {
            total += e - cur_end;
            cur_end = e;
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
    if n == 1 {
        return (per_question[0], per_question[0]);
    }
    let samples = samples.max(1);
    let alpha = alpha.clamp(0.0, 1.0);

    let mut state = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
    let mut means: Vec<f32> = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut sum = 0.0f64;
        for _ in 0..n {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let idx = (state % n as u64) as usize;
            sum += per_question[idx] as f64;
        }
        means.push((sum / n as f64) as f32);
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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
    if low == high {
        sorted[low]
    } else {
        let frac = pos - low as f32;
        sorted[low] + (sorted[high] - sorted[low]) * frac
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
            chunk_set_id: Uuid::nil(),
            sequence: 0,
            heading: String::new(),
            text: String::new(),
            char_start: start,
            char_end: end,
        }
    }

    fn question(refs: &[(u32, u32)]) -> EvaluationQuestion {
        EvaluationQuestion {
            sequence: 0,
            question: "q".into(),
            references: refs
                .iter()
                .map(|&(s, e)| EvaluationReference {
                    content: String::new(),
                    char_start: s,
                    char_end: e,
                    embedding: None,
                })
                .collect(),
            embedding: None,
            category: Default::default(),
            grammar_variant: Default::default(),
            paraphrase_of: None,
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
    fn non_overlapping_len_merges_overlaps() {
        assert_eq!(non_overlapping_len(&[(0, 10), (5, 15), (20, 25)]), 20);
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
        assert!((hw - lw) > (ht - lt), "wide CI should be wider than tight CI");
    }

    #[test]
    fn bootstrap_ci_changes_with_seed() {
        let v = vec![0.1, 0.4, 0.5, 0.7, 0.9, 0.95, 0.2, 0.6, 0.55, 0.8];
        let (l1, _) = bootstrap_ci(&v, 1, 200, 0.05);
        let (l2, _) = bootstrap_ci(&v, 99, 200, 0.05);
        assert!((l1 - l2).abs() > 1e-6, "different seeds should yield different draws");
    }
}
