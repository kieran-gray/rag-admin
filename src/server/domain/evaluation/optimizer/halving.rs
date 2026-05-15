#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBudget {
    Quick,
    Thorough,
    Exhaustive,
}

impl SearchBudget {
    pub fn schedule(self) -> Vec<Rung> {
        match self {
            SearchBudget::Quick => vec![
                Rung::new(24, Fraction::Quarter),
                Rung::new(12, Fraction::Half),
                Rung::new(6, Fraction::Full),
            ],
            SearchBudget::Thorough => vec![
                Rung::new(48, Fraction::Quarter),
                Rung::new(24, Fraction::Half),
                Rung::new(12, Fraction::Full),
                Rung::new(6, Fraction::Full),
            ],
            SearchBudget::Exhaustive => vec![
                Rung::new(96, Fraction::Quarter),
                Rung::new(48, Fraction::Half),
                Rung::new(24, Fraction::Full),
                Rung::new(12, Fraction::Full),
            ],
        }
    }

    pub fn holdout_top_n(self) -> usize {
        match self {
            SearchBudget::Quick => 1,
            SearchBudget::Thorough => 3,
            SearchBudget::Exhaustive => 5,
        }
    }

    pub fn describe(self) -> String {
        let schedule = self.schedule();
        let rung_counts: Vec<String> = schedule.iter().map(|r| r.trials.to_string()).collect();
        format!(
            "{} trials · {} rungs · holdout top {}",
            rung_counts.join(" + "),
            schedule.len(),
            self.holdout_top_n(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fraction {
    Quarter,
    Half,
    Full,
}

impl Fraction {
    pub fn apply(self, total: usize) -> usize {
        match self {
            Fraction::Quarter => total.div_ceil(4).max(1),
            Fraction::Half => total.div_ceil(2).max(1),
            Fraction::Full => total.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rung {
    pub trials: usize,
    pub question_fraction: Fraction,
}

impl Rung {
    pub const fn new(trials: usize, question_fraction: Fraction) -> Self {
        Self {
            trials,
            question_fraction,
        }
    }

    pub fn question_count(self, total: usize) -> usize {
        self.question_fraction.apply(total)
    }
}

pub fn survivors(scored: &[(u32, f32)], take: usize) -> Vec<u32> {
    let mut sorted: Vec<(u32, f32)> = scored.to_vec();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted
        .into_iter()
        .take(take.min(scored.len()))
        .map(|(id, _)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_has_three_rungs_decreasing_trials() {
        let s = SearchBudget::Quick.schedule();
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].trials, 24);
        assert_eq!(s[1].trials, 12);
        assert_eq!(s[2].trials, 6);
    }

    #[test]
    fn thorough_uses_quarter_then_half_then_full_questions() {
        let s = SearchBudget::Thorough.schedule();
        assert_eq!(s.len(), 4);
        assert_eq!(s[0].question_fraction, Fraction::Quarter);
        assert_eq!(s[1].question_fraction, Fraction::Half);
        assert_eq!(s[2].question_fraction, Fraction::Full);
        assert_eq!(s[3].question_fraction, Fraction::Full);
    }

    #[test]
    fn holdout_top_n_grows_with_budget() {
        assert_eq!(SearchBudget::Quick.holdout_top_n(), 1);
        assert_eq!(SearchBudget::Thorough.holdout_top_n(), 3);
        assert_eq!(SearchBudget::Exhaustive.holdout_top_n(), 5);
    }

    #[test]
    fn fraction_applied_to_24_questions() {
        assert_eq!(Fraction::Quarter.apply(24), 6);
        assert_eq!(Fraction::Half.apply(24), 12);
        assert_eq!(Fraction::Full.apply(24), 24);
    }

    #[test]
    fn survivors_picks_top_n_by_score() {
        let scored = vec![(0, 0.1), (1, 0.9), (2, 0.5), (3, 0.7)];
        assert_eq!(survivors(&scored, 2), vec![1, 3]);
    }

    #[test]
    fn budget_describe_matches_optimization_budget() {
        use crate::shared::OptimizationBudget;
        assert_eq!(
            SearchBudget::Quick.describe(),
            OptimizationBudget::Quick.describe()
        );
        assert_eq!(
            SearchBudget::Thorough.describe(),
            OptimizationBudget::Thorough.describe()
        );
        assert_eq!(
            SearchBudget::Exhaustive.describe(),
            OptimizationBudget::Exhaustive.describe()
        );
    }
}
