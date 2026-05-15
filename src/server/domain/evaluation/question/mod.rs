pub mod category;
pub mod variant;

pub use category::QuestionCategory;
pub use variant::GrammarVariant;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationReference {
    pub content: String,
    pub char_start: u32,
    pub char_end: u32,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationQuestion {
    pub sequence: u32,
    pub question: String,
    pub references: Vec<EvaluationReference>,
    pub embedding: Option<Vec<f32>>,
    pub category: QuestionCategory,
    pub grammar_variant: GrammarVariant,
    pub paraphrase_of: Option<u32>,
}

impl From<EvaluationReference> for crate::contracts::EvaluationReferenceDto {
    fn from(r: EvaluationReference) -> Self {
        Self {
            content: r.content,
            char_start: r.char_start,
            char_end: r.char_end,
            embedding: r.embedding.map(crate::core::ordered_f32_vec),
        }
    }
}

impl From<EvaluationQuestion> for crate::contracts::EvaluationQuestionDto {
    fn from(q: EvaluationQuestion) -> Self {
        Self {
            question: q.question,
            references: q.references.into_iter().map(Into::into).collect(),
            embedding: q.embedding.map(crate::core::ordered_f32_vec),
            category: q.category.as_str().into(),
            grammar_variant: q.grammar_variant.as_str().into(),
            paraphrase_of: q.paraphrase_of,
        }
    }
}
