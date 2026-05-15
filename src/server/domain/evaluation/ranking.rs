pub trait Scored {
    fn score(&self) -> f32;
    fn score_ci(&self) -> (f32, f32);
}

pub fn equivalence_class<T: Scored>(entries: &[T]) -> Vec<usize> {
    if entries.is_empty() {
        return Vec::new();
    }
    let leader_idx = entries
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.score()
                .partial_cmp(&b.score())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let (leader_lo, leader_hi) = entries[leader_idx].score_ci();

    entries
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            if *i == leader_idx {
                return true;
            }
            let (lo, hi) = e.score_ci();
            leader_hi.min(hi) >= leader_lo.max(lo)
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct E {
        score: f32,
        lo: f32,
        hi: f32,
    }

    impl Scored for E {
        fn score(&self) -> f32 {
            self.score
        }
        fn score_ci(&self) -> (f32, f32) {
            (self.lo, self.hi)
        }
    }

    fn e(score: f32, lo: f32, hi: f32) -> E {
        E { score, lo, hi }
    }

    #[test]
    fn empty_input_returns_empty() {
        let v: Vec<E> = Vec::new();
        assert!(equivalence_class(&v).is_empty());
    }

    #[test]
    fn single_entry_returns_itself() {
        let v = vec![e(0.5, 0.4, 0.6)];
        assert_eq!(equivalence_class(&v), vec![0]);
    }

    #[test]
    fn leader_alone_when_ci_disjoint() {
        let v = vec![
            e(0.9, 0.85, 0.95),
            e(0.3, 0.25, 0.35),
            e(0.5, 0.45, 0.55),
        ];
        assert_eq!(equivalence_class(&v), vec![0]);
    }

    #[test]
    fn overlapping_ci_joins_class() {
        let v = vec![
            e(0.80, 0.75, 0.85),
            e(0.78, 0.73, 0.83),
            e(0.40, 0.35, 0.45),
        ];
        assert_eq!(equivalence_class(&v), vec![0, 1]);
    }

    #[test]
    fn class_uses_leader_not_input_order() {
        let v = vec![
            e(0.50, 0.45, 0.55),
            e(0.90, 0.85, 0.95),
            e(0.88, 0.83, 0.93),
        ];
        assert_eq!(equivalence_class(&v), vec![1, 2]);
    }

    #[test]
    fn touching_endpoints_count_as_overlap() {
        let v = vec![e(0.8, 0.7, 0.85), e(0.6, 0.55, 0.70)];
        assert_eq!(equivalence_class(&v), vec![0, 1]);
    }
}
