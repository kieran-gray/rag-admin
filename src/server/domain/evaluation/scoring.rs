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
    fn precision_omega_isolates_chunking_quality() {
        let q = question(&[(10, 20)]);
        let chunks = vec![chunk(0, 30), chunk(100, 200)];
        close(precision_omega(&q, &chunks), 10.0 / 30.0, "Pω");
    }

    #[test]
    fn non_overlapping_len_merges_overlaps() {
        assert_eq!(non_overlapping_len(&[(0, 10), (5, 15), (20, 25)]), 20);
    }
}
