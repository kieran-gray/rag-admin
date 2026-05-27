use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::evaluation::{
    dataset::{
        aggregate::DatasetGenerationStatus,
        read_model::{EvaluationDatasetReadModel, NewDatasetSummary},
        repository::{
            DatasetListCursor, DatasetListPage, DatasetListPageItem, DatasetListQuery,
            DatasetStatusFilter, DatasetWithDocumentTitle, EvaluationDatasetRepository,
            EvaluationDatasetRepositoryError,
        },
    },
    question::{
        CognitiveOperation, EvaluationQuestion, EvaluationReference, EvidenceKind,
        QuestionDimensions,
    },
};
use crate::server::domain::shared::Timestamp;
use crate::server::infrastructure::shared::sql::timestamps::to_offset_datetime;

pub struct PostgresEvaluationDatasetRepository {
    pool: PgPool,
}

impl PostgresEvaluationDatasetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const DATASET_COLS: &str = "dataset_id, document_id, document_version, content_hash, label, \
target_question_count, generation_model_id, generation_model, \
duplicate_similarity_threshold_milli, embedding_model_id, status, question_count, \
rejection_count, failure_reason, created_at";

#[async_trait]
impl EvaluationDatasetRepository for PostgresEvaluationDatasetRepository {
    async fn load(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError> {
        let row: Option<DatasetRow> = sqlx::query_as(&format!(
            "SELECT {DATASET_COLS} FROM evaluation_datasets \
             WHERE dataset_id = $1 AND deleted_at IS NULL"
        ))
        .bind(dataset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("load: {e}")))?;

        Ok(row.map(EvaluationDatasetReadModel::from))
    }

    async fn load_with_document_title(
        &self,
        dataset_id: Uuid,
    ) -> Result<Option<DatasetWithDocumentTitle>, EvaluationDatasetRepositoryError> {
        let row = sqlx::query(&format!(
            "SELECT {DATASET_COLS}, document_title FROM evaluation_datasets \
             WHERE dataset_id = $1 AND deleted_at IS NULL"
        ))
        .bind(dataset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("load_with_document_title: {e}"))
        })?;

        let Some(row) = row else { return Ok(None) };
        let dataset_row = DatasetRow {
            dataset_id: row.try_get("dataset_id").map_err(|e| map_row_err(&e))?,
            document_id: row.try_get("document_id").map_err(|e| map_row_err(&e))?,
            document_version: row
                .try_get("document_version")
                .map_err(|e| map_row_err(&e))?,
            content_hash: row.try_get("content_hash").map_err(|e| map_row_err(&e))?,
            label: row.try_get("label").map_err(|e| map_row_err(&e))?,
            target_question_count: row
                .try_get("target_question_count")
                .map_err(|e| map_row_err(&e))?,
            generation_model_id: row
                .try_get("generation_model_id")
                .map_err(|e| map_row_err(&e))?,
            generation_model: row
                .try_get("generation_model")
                .map_err(|e| map_row_err(&e))?,
            duplicate_similarity_threshold_milli: row
                .try_get("duplicate_similarity_threshold_milli")
                .map_err(|e| map_row_err(&e))?,
            embedding_model_id: row
                .try_get("embedding_model_id")
                .map_err(|e| map_row_err(&e))?,
            status: row.try_get("status").map_err(|e| map_row_err(&e))?,
            question_count: row.try_get("question_count").map_err(|e| map_row_err(&e))?,
            rejection_count: row
                .try_get("rejection_count")
                .map_err(|e| map_row_err(&e))?,
            failure_reason: row.try_get("failure_reason").map_err(|e| map_row_err(&e))?,
            created_at: row.try_get("created_at").map_err(|e| map_row_err(&e))?,
        };
        let document_title: Option<String> =
            row.try_get("document_title").map_err(|e| map_row_err(&e))?;
        Ok(Some(DatasetWithDocumentTitle {
            dataset: EvaluationDatasetReadModel::from(dataset_row),
            document_title,
        }))
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationDatasetReadModel>, EvaluationDatasetRepositoryError> {
        let rows: Vec<DatasetRow> = sqlx::query_as(&format!(
            "SELECT {DATASET_COLS} FROM evaluation_datasets \
             WHERE document_id = $1 AND deleted_at IS NULL \
             ORDER BY created_at DESC"
        ))
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

    async fn list_page(
        &self,
        query: &DatasetListQuery,
    ) -> Result<DatasetListPage, EvaluationDatasetRepositoryError> {
        let limit = query.limit.clamp(1, 200) as i64;
        let fetch_limit = limit + 1;

        let total_all: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM evaluation_datasets WHERE deleted_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("count_all: {e}")))?;

        let total_matching: i64 = {
            let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                "SELECT COUNT(*)::BIGINT FROM evaluation_datasets WHERE deleted_at IS NULL",
            );
            push_dataset_status_filter(&mut qb, &query.statuses);
            qb.build_query_scalar::<i64>()
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    EvaluationDatasetRepositoryError::Internal(format!("count_matching: {e}"))
                })?
        };

        let count_rows = sqlx::query(
            "SELECT status, COUNT(*)::BIGINT AS count FROM evaluation_datasets \
             WHERE deleted_at IS NULL GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("status_counts: {e}")))?;

        let mut status_counts: Vec<(DatasetStatusFilter, u64)> = Vec::new();
        for row in count_rows {
            let status: String = row.try_get("status").map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("status row: {e}"))
            })?;
            let count: i64 = row.try_get("count").map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("count row: {e}"))
            })?;
            if let Some(filter) = dataset_status_filter_from_str(&status) {
                status_counts.push((filter, count.max(0) as u64));
            }
        }
        status_counts.sort_by_key(|(f, _)| dataset_status_filter_order(*f));

        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(&format!(
            "SELECT {DATASET_COLS}, d.document_title, d.run_count \
             FROM evaluation_datasets d \
             WHERE d.deleted_at IS NULL"
        ));
        push_dataset_status_filter(&mut qb, &query.statuses);

        if let Some(cursor) = &query.cursor {
            qb.push(" AND (d.created_at, d.dataset_id) < (");
            qb.push_bind(cursor.created_at);
            qb.push(", ");
            qb.push_bind(cursor.dataset_id);
            qb.push(")");
        }
        qb.push(" ORDER BY d.created_at DESC, d.dataset_id DESC LIMIT ");
        qb.push_bind(fetch_limit);

        let rows =
            qb.build().fetch_all(&self.pool).await.map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("list_page: {e}"))
            })?;

        let mut items: Vec<DatasetListPageItem> = Vec::with_capacity(rows.len());
        for row in rows {
            let dataset_row = DatasetRow {
                dataset_id: row.try_get("dataset_id").map_err(|e| map_row_err(&e))?,
                document_id: row.try_get("document_id").map_err(|e| map_row_err(&e))?,
                document_version: row
                    .try_get("document_version")
                    .map_err(|e| map_row_err(&e))?,
                content_hash: row.try_get("content_hash").map_err(|e| map_row_err(&e))?,
                label: row.try_get("label").map_err(|e| map_row_err(&e))?,
                target_question_count: row
                    .try_get("target_question_count")
                    .map_err(|e| map_row_err(&e))?,
                generation_model_id: row
                    .try_get("generation_model_id")
                    .map_err(|e| map_row_err(&e))?,
                generation_model: row
                    .try_get("generation_model")
                    .map_err(|e| map_row_err(&e))?,
                duplicate_similarity_threshold_milli: row
                    .try_get("duplicate_similarity_threshold_milli")
                    .map_err(|e| map_row_err(&e))?,
                embedding_model_id: row
                    .try_get("embedding_model_id")
                    .map_err(|e| map_row_err(&e))?,
                status: row.try_get("status").map_err(|e| map_row_err(&e))?,
                question_count: row.try_get("question_count").map_err(|e| map_row_err(&e))?,
                rejection_count: row
                    .try_get("rejection_count")
                    .map_err(|e| map_row_err(&e))?,
                failure_reason: row.try_get("failure_reason").map_err(|e| map_row_err(&e))?,
                created_at: row.try_get("created_at").map_err(|e| map_row_err(&e))?,
            };
            let document_title: Option<String> =
                row.try_get("document_title").map_err(|e| map_row_err(&e))?;
            let run_count: i32 = row.try_get("run_count").map_err(|e| map_row_err(&e))?;
            items.push(DatasetListPageItem {
                dataset: EvaluationDatasetReadModel::from(dataset_row),
                document_title,
                run_count: run_count.max(0) as u32,
            });
        }

        let next_cursor = if items.len() as i64 > limit {
            items.pop();
            items
                .last()
                .map(
                    |m| -> Result<DatasetListCursor, EvaluationDatasetRepositoryError> {
                        let parsed =
                            OffsetDateTime::parse(&m.dataset.created_at.to_string(), &Rfc3339)
                                .map_err(|e| {
                                    EvaluationDatasetRepositoryError::Internal(format!(
                                        "cursor timestamp: {e}"
                                    ))
                                })?;
                        Ok(DatasetListCursor {
                            created_at: parsed,
                            dataset_id: m.dataset.dataset_id,
                        })
                    },
                )
                .transpose()?
        } else {
            None
        };

        Ok(DatasetListPage {
            items,
            next_cursor,
            total_matching: total_matching.max(0) as u64,
            total_all: total_all.max(0) as u64,
            status_counts,
        })
    }

    async fn load_questions(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationQuestion>, EvaluationDatasetRepositoryError> {
        let question_rows: Vec<QuestionRow> = sqlx::query_as(
            "SELECT sequence, question, embedding, operation, evidence_kind, role, evidence_refs \
             FROM evaluation_questions WHERE dataset_id = $1 ORDER BY sequence ASC",
        )
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("load_questions: {e}")))?;

        let mut questions = Vec::new();
        for q_row in question_rows {
            let ref_rows: Vec<ReferenceRow> = sqlx::query_as(
                "SELECT document_id, content, char_start, char_end, embedding \
                 FROM evaluation_references WHERE dataset_id = $1 AND question_sequence = $2 \
                 ORDER BY sequence ASC",
            )
            .bind(dataset_id)
            .bind(q_row.sequence)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("load_references: {e}"))
            })?;

            let operation = CognitiveOperation::parse(&q_row.operation).map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("operation: {e}"))
            })?;
            let evidence = EvidenceKind::parse(&q_row.evidence_kind).map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("evidence kind: {e}"))
            })?;
            let evidence_refs: Vec<MapItemRef> = serde_json::from_value(q_row.evidence_refs)
                .map_err(|e| {
                    EvaluationDatasetRepositoryError::Internal(format!("evidence_refs: {e}"))
                })?;
            questions.push(EvaluationQuestion {
                sequence: q_row.sequence as u32,
                question: q_row.question,
                references: ref_rows.into_iter().map(Into::into).collect(),
                embedding: q_row.embedding.and_then(|v| serde_json::from_value(v).ok()),
                dimensions: QuestionDimensions {
                    operation,
                    evidence,
                    role: q_row.role,
                },
                evidence_refs,
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
            "
            INSERT INTO evaluation_datasets (
                dataset_id, document_id, document_version, content_hash, label,
                target_question_count, generation_model_id, generation_model,
                duplicate_similarity_threshold_milli,
                embedding_model_id, status, question_count, rejection_count,
                failure_reason, document_title, run_count, created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                'awaiting_map', 0, 0, NULL,
                (SELECT latest_metadata->>'title' FROM source_documents WHERE document_id = $2),
                0,
                $11, NOW()
            )
            ON CONFLICT (dataset_id) DO NOTHING
            ",
        )
        .bind(summary.dataset_id)
        .bind(summary.document_id)
        .bind(summary.document_version as i32)
        .bind(&summary.content_hash)
        .bind(&summary.label)
        .bind(summary.target_question_count as i32)
        .bind(summary.generation_model_id)
        .bind(&summary.generation_model)
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
        let evidence_refs = serde_json::to_value(&question.evidence_refs).map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("serialize evidence_refs: {e}"))
        })?;

        let inserted: (bool,) = sqlx::query_as(
            "
            INSERT INTO evaluation_questions (
                dataset_id, sequence, question, embedding,
                operation, evidence_kind, role, evidence_refs
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (dataset_id, sequence) DO UPDATE SET
                question = EXCLUDED.question,
                embedding = EXCLUDED.embedding,
                operation = EXCLUDED.operation,
                evidence_kind = EXCLUDED.evidence_kind,
                role = EXCLUDED.role,
                evidence_refs = EXCLUDED.evidence_refs
            RETURNING (xmax = 0) AS is_new
            ",
        )
        .bind(dataset_id)
        .bind(question.sequence as i32)
        .bind(&question.question)
        .bind(&embedding)
        .bind(question.dimensions.operation.as_str())
        .bind(question.dimensions.evidence.as_str())
        .bind(&question.dimensions.role)
        .bind(&evidence_refs)
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
                "
                INSERT INTO evaluation_references (
                    dataset_id, question_sequence, sequence, document_id, content,
                    char_start, char_end, embedding
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (dataset_id, question_sequence, sequence) DO UPDATE SET
                    document_id = EXCLUDED.document_id,
                    content = EXCLUDED.content,
                    char_start = EXCLUDED.char_start,
                    char_end = EXCLUDED.char_end,
                    embedding = EXCLUDED.embedding
                ",
            )
            .bind(dataset_id)
            .bind(question.sequence as i32)
            .bind(i as i32)
            .bind(reference.document_id)
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
                "UPDATE evaluation_datasets SET question_count = question_count + 1, \
                 updated_at = NOW() WHERE dataset_id = $1",
            )
            .bind(dataset_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("bump question_count: {e}"))
            })?;
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
            "UPDATE evaluation_datasets SET rejection_count = rejection_count + 1, \
             updated_at = NOW() WHERE dataset_id = $1",
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
            "UPDATE evaluation_datasets SET status = 'completed', failure_reason = NULL, \
             updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_completed: {e}")))?;
        Ok(())
    }

    async fn set_map_status(
        &self,
        dataset_id: Uuid,
        ready: bool,
        failure_reason: Option<String>,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        if ready {
            sqlx::query(
                "UPDATE evaluation_datasets \
                 SET map_ready = TRUE, map_failure_reason = NULL, status = \
                     CASE WHEN status = 'awaiting_map' THEN 'generating' ELSE status END, \
                     updated_at = NOW() \
                 WHERE dataset_id = $1",
            )
            .bind(dataset_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("set_map_status ready: {e}"))
            })?;
        } else {
            let reason = failure_reason.unwrap_or_else(|| "map build failed".into());
            sqlx::query(
                "UPDATE evaluation_datasets \
                 SET map_ready = FALSE, map_failure_reason = $2, status = 'failed', \
                     failure_reason = $2, updated_at = NOW() \
                 WHERE dataset_id = $1",
            )
            .bind(dataset_id)
            .bind(reason)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                EvaluationDatasetRepositoryError::Internal(format!("set_map_status failed: {e}"))
            })?;
        }
        Ok(())
    }

    async fn mark_failed(
        &self,
        dataset_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET status = 'failed', failure_reason = $2, \
             updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_failed: {e}")))?;
        Ok(())
    }

    async fn mark_cancelled(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET status = 'cancelled', \
             updated_at = NOW() WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_cancelled: {e}")))?;
        Ok(())
    }

    async fn rename(
        &self,
        dataset_id: Uuid,
        label: String,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets SET label = $2, updated_at = NOW() \
             WHERE dataset_id = $1",
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
            "UPDATE evaluation_datasets SET deleted_at = NOW(), updated_at = NOW() \
             WHERE dataset_id = $1 AND deleted_at IS NULL",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationDatasetRepositoryError::Internal(format!("mark_deleted: {e}")))?;
        Ok(())
    }

    async fn increment_run_count(
        &self,
        dataset_id: Uuid,
    ) -> Result<(), EvaluationDatasetRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_datasets \
             SET run_count = run_count + 1, updated_at = NOW() \
             WHERE dataset_id = $1",
        )
        .bind(dataset_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("increment_run_count: {e}"))
        })?;
        Ok(())
    }

    async fn find_awaiting_for_map(
        &self,
        map_id: Uuid,
    ) -> Result<Vec<Uuid>, EvaluationDatasetRepositoryError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT d.dataset_id \
             FROM evaluation_datasets d \
             JOIN document_maps m \
               ON m.document_id = d.document_id \
              AND m.document_version = d.document_version \
             WHERE m.map_id = $1 \
               AND d.status = 'awaiting_map' \
               AND d.deleted_at IS NULL",
        )
        .bind(map_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationDatasetRepositoryError::Internal(format!("find_awaiting_for_map: {e}"))
        })?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

fn map_row_err(e: &sqlx::Error) -> EvaluationDatasetRepositoryError {
    EvaluationDatasetRepositoryError::Internal(format!("row: {e}"))
}

fn push_dataset_status_filter(
    qb: &mut QueryBuilder<'_, Postgres>,
    statuses: &[DatasetStatusFilter],
) {
    if statuses.is_empty() {
        return;
    }
    qb.push(" AND status IN (");
    let mut first = true;
    for status in statuses {
        if !first {
            qb.push(", ");
        }
        first = false;
        qb.push_bind(dataset_status_filter_db(*status));
    }
    qb.push(")");
}

fn dataset_status_filter_db(filter: DatasetStatusFilter) -> &'static str {
    match filter {
        DatasetStatusFilter::Completed => "completed",
        DatasetStatusFilter::Generating => "generating",
        DatasetStatusFilter::Failed => "failed",
        DatasetStatusFilter::Cancelled => "cancelled",
    }
}

fn dataset_status_filter_from_str(s: &str) -> Option<DatasetStatusFilter> {
    match s {
        "completed" => Some(DatasetStatusFilter::Completed),
        "generating" | "awaiting_map" => Some(DatasetStatusFilter::Generating),
        "failed" => Some(DatasetStatusFilter::Failed),
        "cancelled" => Some(DatasetStatusFilter::Cancelled),
        _ => None,
    }
}

fn dataset_status_filter_order(filter: DatasetStatusFilter) -> u8 {
    match filter {
        DatasetStatusFilter::Generating => 0,
        DatasetStatusFilter::Completed => 1,
        DatasetStatusFilter::Failed => 2,
        DatasetStatusFilter::Cancelled => 3,
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
    operation: String,
    evidence_kind: String,
    role: String,
    evidence_refs: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct ReferenceRow {
    document_id: Uuid,
    content: String,
    char_start: i32,
    char_end: i32,
    embedding: Option<serde_json::Value>,
}

impl From<ReferenceRow> for EvaluationReference {
    fn from(row: ReferenceRow) -> Self {
        Self {
            document_id: row.document_id,
            content: row.content,
            char_start: row.char_start as u32,
            char_end: row.char_end as u32,
            embedding: row.embedding.and_then(|v| serde_json::from_value(v).ok()),
        }
    }
}
