use async_trait::async_trait;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::server::domain::evaluation::{
    dataset::{
        aggregate::DatasetGenerationStatus,
        read_model::{EvaluationDatasetReadModel, NewDatasetSummary},
        repository::{EvaluationDatasetRepository, EvaluationDatasetRepositoryError},
    },
    question::EvaluationQuestion,
};
use crate::server::domain::shared::Timestamp;
use crate::server::infrastructure::postgres::timestamps::to_offset_datetime;

pub struct PostgresEvaluationDatasetRepository {
    pool: PgPool,
}

impl PostgresEvaluationDatasetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EvaluationDatasetRepository for PostgresEvaluationDatasetRepository {
    async fn load(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError> {
        let row: Option<DatasetRow> = sqlx::query_as(
            r#"
            SELECT
                dataset_id, document_id, document_version, content_hash, label,
                target_question_count, generation_model_id, generation_model,
                excerpt_similarity_threshold_milli, duplicate_similarity_threshold_milli,
                embedding_model_id, status, question_count, rejection_count,
                failure_reason, created_at
            FROM evaluation_datasets
            WHERE dataset_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(dataset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("load: {e}")))?;

        Ok(row.map(EvaluationDatasetReadModel::from))
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError> {
        let rows: Vec<DatasetRow> = sqlx::query_as(
            r#"
            SELECT
                dataset_id, document_id, document_version, content_hash, label,
                target_question_count, generation_model_id, generation_model,
                excerpt_similarity_threshold_milli, duplicate_similarity_threshold_milli,
                embedding_model_id, status, question_count, rejection_count,
                failure_reason, created_at
            FROM evaluation_datasets
            WHERE document_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("list_for_document: {e}"))
        })?;

        Ok(rows
            .into_iter()
            .map(EvaluationDatasetReadModel::from)
            .collect())
    }

    async fn load_questions(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationQuestion>, EvaluationDatasetRepositoryError> {
        let question_rows: Vec<QuestionRow> = sqlx::query_as(
            "SELECT sequence, question, embedding FROM evaluation_questions WHERE dataset_id = $1 ORDER BY sequence ASC"
        )
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("load_questions: {e}")))?;

        let mut questions = Vec::new();
        for q_row in question_rows {
            let ref_rows: Vec<ReferenceRow> = sqlx::query_as(
                "SELECT content, char_start, char_end, embedding FROM evaluation_references WHERE dataset_id = $1 AND question_sequence = $2 ORDER BY sequence ASC"
            )
            .bind(dataset_id)
            .bind(q_row.sequence)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("load_references: {e}")))?;

            questions.push(EvaluationQuestion {
                sequence: q_row.sequence as u32,
                question: q_row.question,
                references: ref_rows.into_iter().map(|r| r.into()).collect(),
                embedding: q_row.embedding.and_then(|v| serde_json::from_value(v).ok()),
            });
        }

        Ok(questions)
    }

    async fn insert_summary(
        &self,
        summary: NewDatasetSummary,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        let created_at = to_offset_datetime(&summary.created_at)
            .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("{e}")))?;

        sqlx::query(
            r#"
            INSERT INTO evaluation_datasets (
                dataset_id, document_id, document_version, content_hash, label,
                target_question_count, generation_model_id, generation_model,
                excerpt_similarity_threshold_milli, duplicate_similarity_threshold_milli,
                embedding_model_id, status, question_count, rejection_count,
                failure_reason, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'generating', 0, 0, NULL, $12, NOW())
            ON CONFLICT (dataset_id) DO NOTHING
            "#,
        )
        .bind(summary.dataset_id)
        .bind(summary.document_id)
        .bind(summary.document_version as i32)
        .bind(&summary.content_hash)
        .bind(&summary.label)
        .bind(summary.target_question_count as i32)
        .bind(summary.generation_model_id)
        .bind(&summary.generation_model)
        .bind(summary.excerpt_similarity_threshold_milli as i32)
        .bind(summary.duplicate_similarity_threshold_milli as i32)
        .bind(summary.embedding_model_id)
        .bind(created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("insert_summary: {e}")))?;

        Ok(())
    }

    async fn save_question(
        &self,
        dataset_id: Uuid,
        question: EvaluationQuestion,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("begin transaction: {e}"))
        })?;

        let embedding = serde_json::to_value(&question.embedding).map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("serialize question embedding: {e}"))
        })?;

        let inserted: (bool,) = sqlx::query_as(
            r#"
            INSERT INTO evaluation_questions (dataset_id, sequence, question, embedding)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (dataset_id, sequence) DO UPDATE SET
                question = EXCLUDED.question,
                embedding = EXCLUDED.embedding
            RETURNING (xmax = 0) AS is_new
            "#,
        )
        .bind(dataset_id)
        .bind(question.sequence as i32)
        .bind(&question.question)
        .bind(&embedding)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("save_question: {e}")))?;

        for (i, reference) in question.references.iter().enumerate() {
            let ref_embedding = serde_json::to_value(&reference.embedding).map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!(
                    "serialize reference embedding: {e}"
                ))
            })?;

            sqlx::query(
                r#"
                INSERT INTO evaluation_references (
                    dataset_id, question_sequence, sequence, content, char_start, char_end, embedding
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (dataset_id, question_sequence, sequence) DO UPDATE SET
                    content = EXCLUDED.content,
                    char_start = EXCLUDED.char_start,
                    char_end = EXCLUDED.char_end,
                    embedding = EXCLUDED.embedding
                "#,
            )
            .bind(dataset_id)
            .bind(question.sequence as i32)
            .bind(i as i32)
            .bind(&reference.content)
            .bind(reference.char_start as i32)
            .bind(reference.char_end as i32)
            .bind(&ref_embedding)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("save_reference: {e}"))
            })?;
        }

        if inserted.0 {
            sqlx::query(
                "UPDATE evaluation_datasets SET question_count = question_count + 1, updated_at = NOW() WHERE dataset_id = $1",
            )
            .bind(dataset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("bump question_count: {e}")))?;
        }

        tx.commit().await.map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("commit transaction: {e}"))
        })?;

        Ok(())
    }

    async fn increment_rejection_count(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET rejection_count = rejection_count + 1, updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("increment_rejection_count: {e}"))
        })?;
        Ok(())
    }

    async fn mark_completed(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET status = 'completed', failure_reason = NULL, updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_completed: {e}")))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        dataset_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET status = 'failed', failure_reason = $2, updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_failed: {e}")))?;
        Ok(())
    }

    async fn rename(
        &self,
        dataset_id: Uuid,
        label: String,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET label = $2, updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .bind(label)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("rename: {e}")))?;
        Ok(())
    }

    async fn mark_deleted(&self, dataset_id: Uuid) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET deleted_at = NOW(), updated_at = NOW() WHERE dataset_id = $1 AND deleted_at IS NULL",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_deleted: {e}")))?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct DatasetRow {
    dataset_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    content_hash: String,
    label: String,
    target_question_count: i32,
    generation_model_id: Uuid,
    generation_model: String,
    excerpt_similarity_threshold_milli: i32,
    duplicate_similarity_threshold_milli: i32,
    embedding_model_id: Uuid,
    status: String,
    question_count: i32,
    rejection_count: i32,
    failure_reason: Option<String>,
    created_at: time::OffsetDateTime,
}

impl From<DatasetRow> for EvaluationDatasetReadModel {
    fn from(row: DatasetRow) -> Self {
        Self {
            dataset_id: row.dataset_id,
            document_id: row.document_id,
            document_version: row.document_version as u32,
            content_hash: row.content_hash,
            label: row.label,
            target_question_count: row.target_question_count as u32,
            generation_model_id: row.generation_model_id,
            generation_model: row.generation_model,
            excerpt_similarity_threshold_milli: row.excerpt_similarity_threshold_milli as u32,
            duplicate_similarity_threshold_milli: row.duplicate_similarity_threshold_milli as u32,
            embedding_model_id: row.embedding_model_id,
            status: DatasetGenerationStatus::from_parts(&row.status, row.failure_reason.clone())
                .unwrap_or(DatasetGenerationStatus::Generating),
            question_count: row.question_count as u32,
            rejection_count: row.rejection_count as u32,
            failure_reason: row.failure_reason,
            created_at: Timestamp::from(row.created_at.format(&Rfc3339).unwrap_or_default()),
        }
    }
}

#[derive(sqlx::FromRow)]
struct QuestionRow {
    sequence: i32,
    question: String,
    embedding: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct ReferenceRow {
    content: String,
    char_start: i32,
    char_end: i32,
    embedding: Option<serde_json::Value>,
}

impl From<ReferenceRow> for crate::server::domain::evaluation::question::EvaluationReference {
    fn from(row: ReferenceRow) -> Self {
        Self {
            content: row.content,
            char_start: row.char_start as u32,
            char_end: row.char_end as u32,
            embedding: row.embedding.and_then(|v| serde_json::from_value(v).ok()),
        }
    }
}
