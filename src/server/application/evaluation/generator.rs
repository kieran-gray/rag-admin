use std::collections::HashMap;

use crate::server::application::evaluation::ports::EvaluationPrompt;
use crate::server::domain::evaluation::question::QuestionCategory;

const FACT_RETRIEVAL: &str = include_str!("prompts/fact_retrieval.txt");
const ARCHITECTURE: &str = include_str!("prompts/architecture.txt");
const REASONING: &str = include_str!("prompts/reasoning.txt");
const CODE: &str = include_str!("prompts/code.txt");
const TRICK: &str = include_str!("prompts/trick.txt");
const GRAMMAR_BREAK: &str = include_str!("prompts/grammar_break.txt");

pub fn build_paraphrase_prompt(original_question: &str) -> EvaluationPrompt {
    EvaluationPrompt {
        system: GRAMMAR_BREAK.to_string(),
        user: format!("Original question:\n{original_question}\n\nReturn only JSON."),
    }
}

pub const PARAPHRASE_MIN_COSINE: f32 = 0.8;

fn system_prompt(category: QuestionCategory) -> &'static str {
    match category {
        QuestionCategory::FactRetrieval => FACT_RETRIEVAL,
        QuestionCategory::Architecture => ARCHITECTURE,
        QuestionCategory::Reasoning => REASONING,
        QuestionCategory::Code => CODE,
        QuestionCategory::Trick => TRICK,
    }
}

pub fn build_question_prompt(
    source_window: &str,
    previous_coverage: &[String],
) -> EvaluationPrompt {
    build_question_prompt_for(
        source_window,
        previous_coverage,
        QuestionCategory::FactRetrieval,
    )
}

pub fn build_question_prompt_for(
    source_window: &str,
    previous_coverage: &[String],
    category: QuestionCategory,
) -> EvaluationPrompt {
    let mut user_prompt = format!("Text:\n{source_window}\n\n");

    if !previous_coverage.is_empty() {
        let previous = previous_coverage.join("\n");
        user_prompt.push_str(&format!(
            "Previously accepted question coverage to avoid repeating:\n{previous}\n\n"
        ));
    };

    user_prompt.push_str(&format!(
        "Category for this question: {}.\n\
         Return only JSON.\n",
        category.label(),
    ));

    EvaluationPrompt {
        system: system_prompt(category).to_string(),
        user: user_prompt,
    }
}

#[derive(Debug, Clone)]
pub struct GenerationPlan {
    pub by_category: HashMap<QuestionCategory, u32>,
    pub grammar_variants_per_question: u32,
}

impl GenerationPlan {
    pub fn default_for_count(target: u32) -> Self {
        let counts = distribute_with_remainder(target, &[300, 250, 200, 150, 100]);
        let mut by_category = HashMap::new();
        for (cat, n) in QuestionCategory::all().iter().zip(counts) {
            by_category.insert(*cat, n);
        }
        Self {
            by_category,
            grammar_variants_per_question: 0,
        }
    }

    pub fn total_questions(&self) -> u32 {
        self.by_category.values().copied().sum()
    }

    pub fn schedule(&self) -> Vec<(QuestionCategory, u32)> {
        QuestionCategory::all()
            .into_iter()
            .map(|c| (c, *self.by_category.get(&c).unwrap_or(&0)))
            .filter(|(_, n)| *n > 0)
            .collect()
    }
}

fn distribute_with_remainder(total: u32, weights_milli: &[u32]) -> Vec<u32> {
    let sum: u64 = weights_milli
        .iter()
        .copied()
        .map(|w| w as u64)
        .sum::<u64>()
        .max(1);
    let total64 = total as u64;
    let mut counts: Vec<u32> = weights_milli
        .iter()
        .map(|w| ((total64 * (*w as u64) + sum / 2) / sum) as u32)
        .collect();
    let assigned: u32 = counts.iter().sum();
    if assigned == total {
        return counts;
    }

    let (idx, _) = weights_milli
        .iter()
        .enumerate()
        .max_by_key(|(_, w)| **w)
        .unwrap_or((0, &0));
    if assigned < total {
        counts[idx] += total - assigned;
    } else {
        counts[idx] = counts[idx].saturating_sub(assigned - total);
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plan_distributes_30_25_20_15_10() {
        let plan = GenerationPlan::default_for_count(100);
        assert_eq!(plan.total_questions(), 100);
        assert_eq!(plan.by_category[&QuestionCategory::FactRetrieval], 30);
        assert_eq!(plan.by_category[&QuestionCategory::Architecture], 25);
        assert_eq!(plan.by_category[&QuestionCategory::Reasoning], 20);
        assert_eq!(plan.by_category[&QuestionCategory::Code], 15);
        assert_eq!(plan.by_category[&QuestionCategory::Trick], 10);
    }

    #[test]
    fn schedule_is_in_canonical_order() {
        let plan = GenerationPlan::default_for_count(10);
        let s = plan.schedule();
        let cats: Vec<QuestionCategory> = s.iter().map(|(c, _)| *c).collect();
        assert_eq!(cats, QuestionCategory::all().to_vec());
    }

    #[test]
    fn small_target_keeps_every_category_at_minimum_zero() {
        let plan = GenerationPlan::default_for_count(1);
        assert_eq!(plan.total_questions(), 1);
    }
}
