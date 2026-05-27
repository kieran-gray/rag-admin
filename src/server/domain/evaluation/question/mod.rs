pub mod dimensions;

pub use dimensions::{
    allowed_evidence_for, is_combination_valid, CognitiveOperation, EvidenceKind,
    QuestionDimensions,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::shared::contracts::{
    EvaluationQuestionDto, EvaluationReferenceDto, QuestionDimensionsDto,
};
use crate::shared::ordered_f32_vec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationReference {
    pub document_id: Uuid,
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
    pub dimensions: QuestionDimensions,
    pub evidence_refs: Vec<MapItemRef>,
}

impl From<EvaluationReference> for EvaluationReferenceDto {
    fn from(r: EvaluationReference) -> Self {
        Self {
            document_id: r.document_id,
            content: r.content,
            char_start: r.char_start,
            char_end: r.char_end,
            embedding: r.embedding.map(ordered_f32_vec),
        }
    }
}

impl From<EvaluationQuestion> for EvaluationQuestionDto {
    fn from(q: EvaluationQuestion) -> Self {
        Self {
            sequence: q.sequence,
            question: q.question,
            references: q.references.into_iter().map(Into::into).collect(),
            embedding: q.embedding.map(ordered_f32_vec),
            dimensions: QuestionDimensionsDto {
                operation: q.dimensions.operation.as_str().into(),
                evidence: q.dimensions.evidence.as_str().into(),
                role: q.dimensions.role,
            },
            evidence_refs: q
                .evidence_refs
                .into_iter()
                .map(super::map_item_ref_to_dto)
                .collect(),
        }
    }
}
