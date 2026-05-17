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

pub fn three_way(seed_source: Uuid, total: usize, ratios: ThreeWayRatios) -> ThreeWaySplit {
    if total < 3 {
        return ThreeWaySplit {
            tuning: Vec::new(),
            validation: Vec::new(),
            holdout: Vec::new(),
        };
    }

    let mut indices: Vec<usize> = (0..total).collect();
    let seed = seed_from_uuid(seed_source);
    shuffle_in_place(&mut indices, seed);

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

pub fn k_fold(seed_source: Uuid, total: usize, k: u32) -> Vec<KFoldEntry> {
    let k = k.max(1);
    if total < k as usize {
        return Vec::new();
    }

    let mut indices: Vec<usize> = (0..total).collect();
    let seed = seed_from_uuid(seed_source);
    shuffle_in_place(&mut indices, seed);

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

pub fn split_questions(
    seed_source: Uuid,
    total: usize,
    tuning_fraction_milli: u32,
) -> DatasetSplit {
    if total == 0 {
        return DatasetSplit {
            tuning: Vec::new(),
            holdout: Vec::new(),
        };
    }

    let mut indices: Vec<usize> = (0..total).collect();
    let seed = seed_from_uuid(seed_source);
    shuffle_in_place(&mut indices, seed);

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

fn seed_from_uuid(id: Uuid) -> u64 {
    let bytes = id.as_bytes();
    let (hi_bytes, lo_bytes) = bytes.split_at(8);
    let mut hi = 0u64;
    let mut lo = 0u64;
    for (b_hi, b_lo) in hi_bytes.iter().zip(lo_bytes.iter()) {
        hi = (hi << 8) | u64::from(*b_hi);
        lo = (lo << 8) | u64::from(*b_lo);
    }
    let seed = hi ^ lo;
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

    #[test]
    fn split_70_30_is_deterministic_per_seed() {
        let id = Uuid::from_u128(0x1234_5678_9abc_def0_1122_3344_5566_7788);
        let a = split_questions(id, 10, 700);
        let b = split_questions(id, 10, 700);
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
        let id = Uuid::from_u128(42);
        let s = split_questions(id, 2, 700);
        assert_eq!(s.tuning.len() + s.holdout.len(), 2);
        assert!(s.is_usable());
    }

    #[test]
    fn split_returns_empty_on_zero() {
        let s = split_questions(Uuid::nil(), 0, 700);
        assert!(s.tuning.is_empty() && s.holdout.is_empty());
        assert!(!s.is_usable());
    }

    #[test]
    fn split_4_questions_gives_3_tuning_1_holdout() {
        let id = Uuid::from_u128(99);
        let s = split_questions(id, 4, 700);
        assert_eq!(s.tuning.len(), 3);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn split_honours_custom_fraction() {
        let id = Uuid::from_u128(99);
        let s = split_questions(id, 10, 500);
        assert_eq!(s.tuning.len(), 5);
        assert_eq!(s.holdout.len(), 5);
    }

    #[test]
    fn split_clamps_fraction_above_1000() {
        let id = Uuid::from_u128(99);
        let s = split_questions(id, 10, 2000);
        assert_eq!(s.tuning.len(), 9);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn three_way_default_ratios_partition_dataset() {
        let id = Uuid::from_u128(0xDEAD_BEEF_CAFE_F00D_1122_3344_5566_7788);
        let s = three_way(id, 50, ThreeWayRatios::default());
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
        let id = Uuid::from_u128(7);
        let a = three_way(id, 25, ThreeWayRatios::default());
        let b = three_way(id, 25, ThreeWayRatios::default());
        assert_eq!(a.tuning, b.tuning);
        assert_eq!(a.validation, b.validation);
        assert_eq!(a.holdout, b.holdout);
    }

    #[test]
    fn three_way_returns_empty_for_tiny_datasets() {
        let id = Uuid::from_u128(7);
        let s = three_way(id, 2, ThreeWayRatios::default());
        assert!(!s.is_usable());
    }

    #[test]
    fn three_way_always_reserves_one_per_partition() {
        let id = Uuid::from_u128(7);
        let s = three_way(id, 3, ThreeWayRatios::default());
        assert!(s.is_usable());
        assert_eq!(s.tuning.len(), 1);
        assert_eq!(s.validation.len(), 1);
        assert_eq!(s.holdout.len(), 1);
    }

    #[test]
    fn three_way_renormalises_unscaled_ratios() {
        let id = Uuid::from_u128(11);
        let s = three_way(
            id,
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
        let id = Uuid::from_u128(0xC0DE);
        let folds = k_fold(id, 23, 5);
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
        let id = Uuid::from_u128(0xC0DE);
        for fold in k_fold(id, 20, 4) {
            let train: std::collections::HashSet<usize> = fold.train.iter().copied().collect();
            for v in &fold.validate {
                assert!(!train.contains(v), "fold {} leaked", fold.fold_index);
            }
            assert_eq!(train.len() + fold.validate.len(), 20);
        }
    }

    #[test]
    fn k_fold_returns_empty_when_too_small() {
        assert!(k_fold(Uuid::from_u128(1), 3, 5).is_empty());
    }
}
