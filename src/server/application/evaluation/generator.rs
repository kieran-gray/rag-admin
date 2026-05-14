use crate::server::application::evaluation::ports::EvaluationPrompt;

const SYSTEM_PROMPT: &str = include_str!("prompts/synthetic_dataset_prompt.txt");

pub fn build_question_prompt(
    source_window: &str,
    previous_coverage: &[String],
) -> EvaluationPrompt {
    let mut user_prompt = format!("Text:\n{source_window}\n\n");

    if !previous_coverage.is_empty() {
        let previous = previous_coverage.join("\n");
        user_prompt.push_str(&format!(
            "Previously accepted question coverage to avoid repeating:\n{previous}\n\n"
        ));
    };

    user_prompt.push_str(
        "Return only JSON.\n\
         Every reference must be copied verbatim from one contiguous span in the text.\n\
         Do not paraphrase, normalize punctuation, remove markdown markers, merge separate passages, or use ellipses.\n\
         If a candidate question would require non-verbatim references, choose a different question.",
    );

    EvaluationPrompt {
        system: SYSTEM_PROMPT.to_string(),
        user: user_prompt,
    }
}
