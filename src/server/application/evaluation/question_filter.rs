use crate::server::application::embedding::{EmbeddingService, ResolvedEmbeddingModel};
use crate::server::application::AppError;
use crate::shared::{ordered_f32_vec, EvaluationQuestionDto};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuestionFilterStats {
    pub low_excerpt_similarity: u32,
    pub duplicate: u32,
}

pub enum QuestionFilterDecision {
    Accepted { kept: usize },
    RejectedLowExcerptSimilarity { similarity: f32 },
    RejectedDuplicate { similarity: f32 },
}

pub struct GeneratedQuestionGate<'a> {
    embedding_service: &'a EmbeddingService,
    embedding_model: &'a ResolvedEmbeddingModel,
    excerpt_similarity_threshold: f32,
    duplicate_similarity_threshold: f32,
    questions: Vec<EvaluationQuestionDto>,
    question_embeddings: Vec<Vec<f32>>,
    stats: QuestionFilterStats,
    generated_count: usize,
}

pub async fn filter_generated_questions(
    embedding_service: &EmbeddingService,
    model: &ResolvedEmbeddingModel,
    questions: Vec<EvaluationQuestionDto>,
    excerpt_similarity_threshold: f32,
    duplicate_similarity_threshold: f32,
) -> Result<(Vec<EvaluationQuestionDto>, QuestionFilterStats), AppError> {
    if questions.is_empty() {
        return Ok((questions, QuestionFilterStats::default()));
    }

    let question_texts: Vec<String> = questions.iter().map(|q| q.question.clone()).collect();
    let question_embeddings = embedding_service
        .embed_with_resolved(model, &question_texts)
        .await?;

    let mut reference_texts = Vec::new();
    let mut reference_question_indexes = Vec::new();
    for (question_index, question) in questions.iter().enumerate() {
        for reference in &question.references {
            reference_texts.push(reference.content.clone());
            reference_question_indexes.push(question_index);
        }
    }

    let reference_embeddings = if reference_texts.is_empty() {
        Vec::new()
    } else {
        embedding_service
            .embed_with_resolved(model, &reference_texts)
            .await?
    };

    Ok(filter_questions_by_embeddings(
        questions,
        &question_embeddings,
        &reference_embeddings,
        &reference_question_indexes,
        excerpt_similarity_threshold,
        duplicate_similarity_threshold,
    ))
}

impl<'a> GeneratedQuestionGate<'a> {
    pub fn new(
        embedding_service: &'a EmbeddingService,
        embedding_model: &'a ResolvedEmbeddingModel,
        excerpt_similarity_threshold: f32,
        duplicate_similarity_threshold: f32,
    ) -> Self {
        Self {
            embedding_service,
            embedding_model,
            excerpt_similarity_threshold,
            duplicate_similarity_threshold,
            questions: Vec::new(),
            question_embeddings: Vec::new(),
            stats: QuestionFilterStats::default(),
            generated_count: 0,
        }
    }

    pub async fn try_accept(
        &mut self,
        question: EvaluationQuestionDto,
    ) -> Result<QuestionFilterDecision, AppError> {
        self.generated_count += 1;

        let mut texts = Vec::with_capacity(question.references.len() + 1);
        texts.push(question.question.clone());
        texts.extend(
            question
                .references
                .iter()
                .map(|reference| reference.content.clone()),
        );

        let embeddings = self
            .embedding_service
            .embed_with_resolved(self.embedding_model, &texts)
            .await?;
        let question_embedding = embeddings.first().cloned();

        match classify_candidate(
            question_embedding.as_deref(),
            &embeddings[1..],
            &self.question_embeddings,
            self.excerpt_similarity_threshold,
            self.duplicate_similarity_threshold,
        ) {
            CandidateClassification::Accepted => {
                let question_embedding =
                    question_embedding.expect("accepted candidate has embedding");
                let mut question = question;
                question.embedding = Some(ordered_f32_vec(question_embedding.clone()));
                for (reference, reference_embedding) in question
                    .references
                    .iter_mut()
                    .zip(embeddings.iter().skip(1))
                {
                    reference.embedding = Some(ordered_f32_vec(reference_embedding.clone()));
                }

                self.question_embeddings.push(question_embedding);
                self.questions.push(question);
                Ok(QuestionFilterDecision::Accepted {
                    kept: self.questions.len(),
                })
            }
            CandidateClassification::RejectedLowExcerptSimilarity { similarity } => {
                self.stats.low_excerpt_similarity += 1;
                Ok(QuestionFilterDecision::RejectedLowExcerptSimilarity { similarity })
            }
            CandidateClassification::RejectedDuplicate { similarity } => {
                self.stats.duplicate += 1;
                Ok(QuestionFilterDecision::RejectedDuplicate { similarity })
            }
        }
    }

    pub fn kept_count(&self) -> usize {
        self.questions.len()
    }

    pub fn generated_count(&self) -> usize {
        self.generated_count
    }

    pub fn stats(&self) -> QuestionFilterStats {
        self.stats
    }

    pub fn latest_question(&self) -> Option<&EvaluationQuestionDto> {
        self.questions.last()
    }

    pub fn into_questions(mut self, target_questions: usize) -> Vec<EvaluationQuestionDto> {
        self.questions.truncate(target_questions);
        self.questions
    }
}

fn filter_questions_by_embeddings(
    questions: Vec<EvaluationQuestionDto>,
    question_embeddings: &[Vec<f32>],
    reference_embeddings: &[Vec<f32>],
    reference_question_indexes: &[usize],
    excerpt_similarity_threshold: f32,
    duplicate_similarity_threshold: f32,
) -> (Vec<EvaluationQuestionDto>, QuestionFilterStats) {
    let mut references_by_question = vec![Vec::new(); questions.len()];
    for (reference_index, question_index) in reference_question_indexes.iter().copied().enumerate()
    {
        if let Some(reference_indexes) = references_by_question.get_mut(question_index) {
            reference_indexes.push(reference_index);
        }
    }

    let mut kept = Vec::new();
    let mut kept_embedding_indexes: Vec<usize> = Vec::new();
    let mut stats = QuestionFilterStats::default();

    for (question_index, question) in questions.into_iter().enumerate() {
        let reference_embeddings_for_question = references_by_question[question_index]
            .iter()
            .filter_map(|i| reference_embeddings.get(*i))
            .cloned()
            .collect::<Vec<_>>();
        let kept_embeddings = kept_embedding_indexes
            .iter()
            .filter_map(|kept_index| question_embeddings.get(*kept_index))
            .cloned()
            .collect::<Vec<_>>();

        match classify_candidate(
            question_embeddings.get(question_index).map(Vec::as_slice),
            &reference_embeddings_for_question,
            &kept_embeddings,
            excerpt_similarity_threshold,
            duplicate_similarity_threshold,
        ) {
            CandidateClassification::Accepted => {
                kept_embedding_indexes.push(question_index);
                kept.push(question);
            }
            CandidateClassification::RejectedLowExcerptSimilarity { .. } => {
                stats.low_excerpt_similarity += 1;
            }
            CandidateClassification::RejectedDuplicate { .. } => {
                stats.duplicate += 1;
            }
        }
    }

    (kept, stats)
}

enum CandidateClassification {
    Accepted,
    RejectedLowExcerptSimilarity { similarity: f32 },
    RejectedDuplicate { similarity: f32 },
}

fn classify_candidate(
    question_embedding: Option<&[f32]>,
    reference_embeddings: &[Vec<f32>],
    kept_embeddings: &[Vec<f32>],
    excerpt_similarity_threshold: f32,
    duplicate_similarity_threshold: f32,
) -> CandidateClassification {
    let Some(question_embedding) = question_embedding else {
        return CandidateClassification::RejectedLowExcerptSimilarity { similarity: 0.0 };
    };
    if reference_embeddings.is_empty() {
        return CandidateClassification::RejectedLowExcerptSimilarity { similarity: 0.0 };
    }

    let min_reference_similarity = reference_embeddings
        .iter()
        .map(|reference_embedding| cosine_similarity(question_embedding, reference_embedding))
        .fold(f32::INFINITY, f32::min);
    if !min_reference_similarity.is_finite()
        || min_reference_similarity < excerpt_similarity_threshold
    {
        return CandidateClassification::RejectedLowExcerptSimilarity {
            similarity: min_reference_similarity.max(0.0),
        };
    }

    let max_duplicate_similarity = kept_embeddings
        .iter()
        .map(|kept_embedding| cosine_similarity(question_embedding, kept_embedding))
        .fold(0.0, f32::max);
    if max_duplicate_similarity >= duplicate_similarity_threshold {
        return CandidateClassification::RejectedDuplicate {
            similarity: max_duplicate_similarity,
        };
    }

    CandidateClassification::Accepted
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::EvaluationReferenceDto;

    fn question(text: &str, reference: &str) -> EvaluationQuestionDto {
        EvaluationQuestionDto {
            question: text.into(),
            references: vec![EvaluationReferenceDto {
                content: reference.into(),
                char_start: 0,
                char_end: reference.chars().count() as u32,
                embedding: None,
            }],
            embedding: None,
        }
    }

    #[test]
    fn filters_questions_with_low_reference_similarity() {
        let questions = vec![
            question("aligned?", "aligned reference"),
            question("unrelated?", "unrelated reference"),
        ];
        let question_embeddings = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
        let reference_embeddings = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let reference_question_indexes = vec![0, 1];

        let (kept, stats) = filter_questions_by_embeddings(
            questions,
            &question_embeddings,
            &reference_embeddings,
            &reference_question_indexes,
            0.5,
            0.95,
        );

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].question, "aligned?");
        assert_eq!(stats.low_excerpt_similarity, 1);
        assert_eq!(stats.duplicate, 0);
    }

    #[test]
    fn filters_duplicate_questions_after_reference_similarity() {
        let questions = vec![
            question("first?", "first reference"),
            question("duplicate?", "duplicate reference"),
            question("different?", "different reference"),
        ];
        let question_embeddings = vec![vec![1.0, 0.0], vec![0.99, 0.01], vec![0.0, 1.0]];
        let reference_embeddings = vec![vec![1.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];
        let reference_question_indexes = vec![0, 1, 2];

        let (kept, stats) = filter_questions_by_embeddings(
            questions,
            &question_embeddings,
            &reference_embeddings,
            &reference_question_indexes,
            0.5,
            0.95,
        );

        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].question, "first?");
        assert_eq!(kept[1].question, "different?");
        assert_eq!(stats.low_excerpt_similarity, 0);
        assert_eq!(stats.duplicate, 1);
    }

    #[test]
    fn classification_rejects_missing_references() {
        let decision = classify_candidate(Some(&[1.0, 0.0]), &[], &[], 0.5, 0.95);

        assert!(matches!(
            decision,
            CandidateClassification::RejectedLowExcerptSimilarity { similarity: 0.0 }
        ));
    }

    #[test]
    fn classification_rejects_duplicates() {
        let decision = classify_candidate(
            Some(&[1.0, 0.0]),
            &[vec![1.0, 0.0]],
            &[vec![0.99, 0.01]],
            0.5,
            0.95,
        );

        assert!(matches!(
            decision,
            CandidateClassification::RejectedDuplicate { .. }
        ));
    }
}
