use std::collections::HashSet;

use uuid::Uuid;

pub struct DatasetSplit {
    pub tuning: Vec<usize>,
    pub holdout: Vec<usize>,
}

impl DatasetSplit {
    pub fn is_usable(&self) -> bool {
        !self.tuning.is_empty() && !self.holdout.is_empty()
    }
}

pub struct ThreeWaySplit {
    pub tuning: Vec<usize>,
    pub validation: Vec<usize>,
    pub holdout: Vec<usize>,
}

impl ThreeWaySplit {
    pub fn is_usable(&self) -> bool {
        !self.tuning.is_empty() && !self.validation.is_empty() && !self.holdout.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreeWayRatios {
    pub tuning_milli: u32,
    pub validation_milli: u32,
    pub holdout_milli: u32,
}

impl Default for ThreeWayRatios {
    fn default() -> Self {
        Self {
            tuning_milli: 600,
            validation_milli: 200,
            holdout_milli: 200,
        }
    }
}

pub struct KFoldEntry {
    pub fold_index: u32,

    pub train: Vec<usize>,

    pub validate: Vec<usize>,
}

pub fn three_way(seed: u64, total: usize, ratios: ThreeWayRatios) -> ThreeWaySplit {
    if total < 3 {
        return ThreeWaySplit {
            tuning: Vec::new(),
            validation: Vec::new(),
            holdout: Vec::new(),
        };
    }

    let mut indices: Vec<usize> = (0..total).collect();
    shuffle_in_place(&mut indices, normalise_seed(seed));

    let sum = (ratios.tuning_milli + ratios.validation_milli + ratios.holdout_milli).max(1) as u64;
    let t = ratios.tuning_milli as u64;
    let v = ratios.validation_milli as u64;

    let total_u64 = total as u64;
    let mut tuning_size = ((total_u64 * t + sum / 2) / sum) as usize;
    let mut validation_size = ((total_u64 * v + sum / 2) / sum) as usize;
    tuning_size = tuning_size.max(1);
    validation_size = validation_size.max(1);
    if tuning_size + validation_size >= total {
        if tuning_size > validation_size {
            tuning_size = total - validation_size - 1;
        } else {
            validation_size = total - tuning_size - 1;
        }
    }
    let holdout_size = total - tuning_size - validation_size;
    debug_assert!(holdout_size >= 1);

    let mut tuning = indices.get(..tuning_size).unwrap_or_default().to_vec();
    let mut validation = indices
        .get(tuning_size..tuning_size + validation_size)
        .unwrap_or_default()
        .to_vec();
    let mut holdout = indices
        .get(tuning_size + validation_size..)
        .unwrap_or_default()
        .to_vec();
    tuning.sort_unstable();
    validation.sort_unstable();
    holdout.sort_unstable();

    ThreeWaySplit {
        tuning,
        validation,
        holdout,
    }
}

pub fn k_fold(seed: u64, total: usize, k: u32) -> Vec<KFoldEntry> {
    let k = k.max(1);
    if total < k as usize {
        return Vec::new();
    }

    let mut indices: Vec<usize> = (0..total).collect();
    shuffle_in_place(&mut indices, normalise_seed(seed));

    let base = total / k as usize;
    let extras = total % k as usize;

    let mut out = Vec::with_capacity(k as usize);
    let mut cursor = 0usize;
    for fold in 0..k as usize {
        let size = base + if fold < extras { 1 } else { 0 };
        let validate_slice: Vec<usize> = indices
            .get(cursor..cursor + size)
            .unwrap_or_default()
            .to_vec();
        cursor += size;

        let validate_set: HashSet<usize> = validate_slice.iter().copied().collect();
        let mut train: Vec<usize> = (0..total).filter(|i| !validate_set.contains(i)).collect();
        let mut validate = validate_slice;
        train.sort_unstable();
        validate.sort_unstable();

        out.push(KFoldEntry {
            fold_index: fold as u32,
            train,
            validate,
        });
    }
    out
}

pub fn split_questions(seed: u64, total: usize, tuning_fraction_milli: u32) -> DatasetSplit {
    if total == 0 {
        return DatasetSplit {
            tuning: Vec::new(),
            holdout: Vec::new(),
        };
    }

    let mut indices: Vec<usize> = (0..total).collect();
    shuffle_in_place(&mut indices, normalise_seed(seed));

    let fraction = tuning_fraction_milli.min(1000) as u64;
    let tuning_size = ((total as u64 * fraction + 500) / 1000) as usize;
    let tuning_size = tuning_size.clamp(1, total.saturating_sub(1).max(1));

    let (tuning, holdout) = indices.split_at(tuning_size);
    let mut tuning = tuning.to_vec();
    let mut holdout = holdout.to_vec();
    tuning.sort_unstable();
    holdout.sort_unstable();
    DatasetSplit { tuning, holdout }
}

pub fn seed_from_uuid(id: Uuid) -> u64 {
    let bytes = id.as_bytes();
    let (hi_bytes, lo_bytes) = bytes.split_at(8);
    let mut hi = 0u64;
    let mut lo = 0u64;
    for (b_hi, b_lo) in hi_bytes.iter().zip(lo_bytes.iter()) {
        hi = (hi << 8) | u64::from(*b_hi);
        lo = (lo << 8) | u64::from(*b_lo);
    }
    normalise_seed(hi ^ lo)
}

fn normalise_seed(seed: u64) -> u64 {
    if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    }
}

fn shuffle_in_place<T>(values: &mut [T], mut state: u64) {
    for i in (1..values.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        values.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(n: u128) -> u64 {
        seed_from_uuid(Uuid::from_u128(n))
    }

    #[test]
    fn split_70_30_is_deterministic_per_seed() {
        let s = seed(0x1234_5678_9abc_def0_1122_3344_5566_7788);
        let a = split_questions(s, 10, 700);
        let b = split_questions(s, 10, 700);
        assert_eq!(a.tuning, b.tuning);
        assert_eq!(a.holdout, b.holdout);
        assert_eq!(a.tuning.len(), 7);
        assert_eq!(a.holdout.len(), 3);
        let mut all: Vec<usize> = a.tuning.iter().chain(a.holdout.iter()).copied().collect();
        all.sort_unstable();
        assert_eq!(all, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn split_keeps_at_least_one_in_each_side() {
        let s = split_questions(seed(42), 2, 700);
        assert_eq!(s.tuning.len() + s.holdout.len(), 2);
        assert!(s.is_usable());
    }

    #[test]
    fn split_returns_empty_on_zero() {
        let s = split_questions(0, 0, 700);
        assert!(s.tuning.is_empty() && s.holdout.is_empty());
        assert!(!s.is_usable());
    }

    #[test]
    fn split_4_questions_gives_3_tuning_1_holdout() {
        let s = split_questions(seed(99), 4, 700);
        assert_eq!(s.tuning.len(), 3);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn split_honours_custom_fraction() {
        let s = split_questions(seed(99), 10, 500);
        assert_eq!(s.tuning.len(), 5);
        assert_eq!(s.holdout.len(), 5);
    }

    #[test]
    fn split_clamps_fraction_above_1000() {
        let s = split_questions(seed(99), 10, 2000);
        assert_eq!(s.tuning.len(), 9);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn explicit_seed_overrides_uuid_derivation() {
        let from_uuid = split_questions(seed(7), 10, 700);
        let from_raw = split_questions(42, 10, 700);
        let from_raw_again = split_questions(42, 10, 700);
        assert_eq!(from_raw.tuning, from_raw_again.tuning);
        assert_ne!(from_uuid.tuning, from_raw.tuning);
    }

    #[test]
    fn three_way_default_ratios_partition_dataset() {
        let s = three_way(
            seed(0xDEAD_BEEF_CAFE_F00D_1122_3344_5566_7788),
            50,
            ThreeWayRatios::default(),
        );
        assert!(s.is_usable());
        assert_eq!(s.tuning.len() + s.validation.len() + s.holdout.len(), 50);

        assert_eq!(s.tuning.len(), 30);
        assert_eq!(s.validation.len(), 10);
        assert_eq!(s.holdout.len(), 10);
        let mut all: Vec<usize> = s
            .tuning
            .iter()
            .chain(s.validation.iter())
            .chain(s.holdout.iter())
            .copied()
            .collect();
        all.sort_unstable();
        assert_eq!(all, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn three_way_is_deterministic_per_seed() {
        let s = seed(7);
        let a = three_way(s, 25, ThreeWayRatios::default());
        let b = three_way(s, 25, ThreeWayRatios::default());
        assert_eq!(a.tuning, b.tuning);
        assert_eq!(a.validation, b.validation);
        assert_eq!(a.holdout, b.holdout);
    }

    #[test]
    fn three_way_returns_empty_for_tiny_datasets() {
        let s = three_way(seed(7), 2, ThreeWayRatios::default());
        assert!(!s.is_usable());
    }

    #[test]
    fn three_way_always_reserves_one_per_partition() {
        let s = three_way(seed(7), 3, ThreeWayRatios::default());
        assert!(s.is_usable());
        assert_eq!(s.tuning.len(), 1);
        assert_eq!(s.validation.len(), 1);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn three_way_renormalises_unscaled_ratios() {
        let s = three_way(
            seed(11),
            20,
            ThreeWayRatios {
                tuning_milli: 6,
                validation_milli: 2,
                holdout_milli: 2,
            },
        );
        assert_eq!(s.tuning.len(), 12);
        assert_eq!(s.validation.len(), 4);
        assert_eq!(s.holdout.len(), 4);
    }

    #[test]
    fn k_fold_partitions_validation_disjointly() {
        let folds = k_fold(seed(0xC0DE), 23, 5);
        assert_eq!(folds.len(), 5);
        let mut sizes: Vec<usize> = folds.iter().map(|f| f.validate.len()).collect();
        sizes.sort_unstable();

        assert_eq!(sizes, vec![4, 4, 5, 5, 5]);
        let mut union: Vec<usize> = folds
            .iter()
            .flat_map(|f| f.validate.iter().copied())
            .collect();
        union.sort_unstable();
        assert_eq!(union, (0..23).collect::<Vec<_>>());
    }

    #[test]
    fn k_fold_train_excludes_validate_per_fold() {
        for fold in k_fold(seed(0xC0DE), 20, 4) {
            let train: std::collections::HashSet<usize> = fold.train.iter().copied().collect();
            for v in &fold.validate {
                assert!(!train.contains(v), "fold {} leaked", fold.fold_index);
            }
            assert_eq!(train.len() + fold.validate.len(), 20);
        }
    }

    #[test]
    fn k_fold_returns_empty_when_too_small() {
        assert!(k_fold(seed(1), 3, 5).is_empty());
    }

    #[test]
    fn seed_from_uuid_nil_is_normalised() {
        let s = seed_from_uuid(Uuid::nil());
        assert_ne!(s, 0, "nil uuid must be normalised away from zero");
    }

    #[test]
    fn seed_from_uuid_collapses_when_halves_are_xor_identical() {
        let mut bytes = [0u8; 16];
        for i in 0..8 {
            bytes[i] = 0x42;
            bytes[i + 8] = 0x42;
        }
        let id = Uuid::from_bytes(bytes);
        let s = seed_from_uuid(id);
        assert_ne!(
            s, 0,
            "symmetric halves XOR to 0, but normalise_seed must produce a usable seed",
        );
    }

    #[test]
    fn split_questions_total_indices_cover_input() {
        for n in [1usize, 5, 10, 50, 100] {
            let s = split_questions(seed(n as u128), n, 700);
            let mut all: Vec<usize> = s.tuning.iter().chain(s.holdout.iter()).copied().collect();
            all.sort_unstable();
            assert_eq!(
                all,
                (0..n).collect::<Vec<_>>(),
                "split must partition indices for n={n}",
            );
        }
    }

    #[test]
    fn split_questions_tuning_holdout_are_disjoint() {
        for n in [3usize, 7, 25, 100] {
            let s = split_questions(seed(n as u128), n, 500);
            let tuning: std::collections::HashSet<usize> = s.tuning.iter().copied().collect();
            for h in &s.holdout {
                assert!(!tuning.contains(h), "n={n}: holdout {h} leaked into tuning");
            }
        }
    }

    #[test]
    fn three_way_indices_partition_input() {
        let s = three_way(seed(0xC0DE), 40, ThreeWayRatios::default());
        let mut all: Vec<usize> = s
            .tuning
            .iter()
            .chain(s.validation.iter())
            .chain(s.holdout.iter())
            .copied()
            .collect();
        all.sort_unstable();
        assert_eq!(all, (0..40).collect::<Vec<_>>());
    }

    #[test]
    fn three_way_zero_ratios_still_partition_safely() {
        let s = three_way(
            seed(7),
            12,
            ThreeWayRatios {
                tuning_milli: 0,
                validation_milli: 0,
                holdout_milli: 0,
            },
        );
        let total = s.tuning.len() + s.validation.len() + s.holdout.len();
        assert_eq!(total, 12, "all indices must still be assigned somewhere");
        assert!(
            s.is_usable(),
            "even with degenerate ratios, each side keeps ≥1"
        );
    }

    #[test]
    fn k_fold_validate_sets_are_disjoint_and_cover_input() {
        let folds = k_fold(seed(0xC0FFEE), 17, 4);
        let mut seen = std::collections::HashSet::<usize>::new();
        for f in &folds {
            for v in &f.validate {
                assert!(seen.insert(*v), "index {v} appeared in two validate sets");
            }
        }
        assert_eq!(seen.len(), 17);
    }

    #[test]
    fn k_fold_train_unions_to_complement_of_validate() {
        for f in k_fold(seed(0xC0FFEE), 20, 5) {
            let train: std::collections::BTreeSet<usize> = f.train.iter().copied().collect();
            let validate: std::collections::BTreeSet<usize> = f.validate.iter().copied().collect();
            assert!(train.is_disjoint(&validate));
            let union: std::collections::BTreeSet<usize> =
                train.union(&validate).copied().collect();
            assert_eq!(union.len(), 20);
        }
    }

    #[test]
    fn k_fold_with_k_zero_treated_as_one() {
        let folds = k_fold(seed(1), 5, 0);
        assert_eq!(folds.len(), 1);
        assert_eq!(folds[0].validate.len(), 5);
        assert!(folds[0].train.is_empty());
    }

    #[test]
    fn split_questions_below_total_keeps_one_holdout() {
        let s = split_questions(seed(13), 5, 1000);
        assert_eq!(s.tuning.len(), 4);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn split_is_deterministic_across_repeated_calls_with_same_seed() {
        let a = split_questions(seed(42), 30, 700);
        let b = split_questions(seed(42), 30, 700);
        assert_eq!(a.tuning, b.tuning);
        assert_eq!(a.holdout, b.holdout);
    }
}
