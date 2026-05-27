use crate::server::application::evaluation::dataset::seed_planner::QuestionSeed;
use crate::server::application::evaluation::ports::EvaluationPrompt;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::evaluation::question::CognitiveOperation;

const PROMPT_TEMPLATE: &str = include_str!("prompts/question_from_seed.txt");

pub fn build_question_prompt(seed: &QuestionSeed, role: &SuggestedRole) -> EvaluationPrompt {
    let seed_items_str = if seed.items.is_empty() {
        if seed.anti_evidence.is_empty() {
            "(no evidence)".to_string()
        } else {
            seed.anti_evidence
                .iter()
                .map(|it| format!("- [{kind}] {summary}", kind = it.kind, summary = it.summary))
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        seed.items
            .iter()
            .map(|it| format!("- [{kind}] {summary}", kind = it.kind, summary = it.summary))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let adversarial_clause = if matches!(seed.dimensions.operation, CognitiveOperation::Adversarial)
    {
        "- The question MUST NOT be answerable from the source document. It should look plausible but the source does not actually contain the answer."
    } else {
        ""
    };

    let user = PROMPT_TEMPLATE
        .replace("{role_name}", &role.name)
        .replace("{role_focus}", &role.focus)
        .replace("{operation_label}", seed.dimensions.operation.label())
        .replace(
            "{operation_definition}",
            seed.dimensions.operation.definition(),
        )
        .replace("{seed_items}", &seed_items_str)
        .replace("{adversarial_clause}", adversarial_clause);

    EvaluationPrompt {
        system: "You are writing a single reading-comprehension question. Return only JSON.".into(),
        user,
    }
}
