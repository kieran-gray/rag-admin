use async_trait::async_trait;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::server::domain::evaluation::run::aggregate::EvaluationRunStatus;
use crate::server::domain::evaluation::run::read_model::{
    EvaluationRunReadModel, EvaluationVariantResultDto, NewRunSummary,
};
use crate::server::domain::evaluation::run::repository::{
    EvaluationRunRepository, EvaluationRunRepositoryError,
};
use crate::server::domain::evaluation::run::scoring_policy::{ScoringPolicy, ScoringWeights};
use crate::server::domain::shared::Timestamp;
use crate::server::infrastructure::postgres::timestamps::to_offset_datetime;
use crate::shared::{
    ChunkingVariant, EvaluationAutotuneRequest, EvaluationResultSplit, EvaluationRunOptions,
};

pub struct PostgresEvaluationRunRepository {
    pool: PgPool,
}

impl PostgresEvaluationRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EvaluationRunRepository for PostgresEvaluationRunRepository {
    async fn load(
        &self,
        run_id: Uuid,
    ) -> Result<Option<EvaluationRunReadModel>, EvaluationRunRepositoryError> {
        let row: Option<RunRow> = sqlx::query_as(
            r#"
            SELECT
                run_id, dataset_id, pipeline_configuration_id, document_id, document_version,
                variants, options, autotune_request,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, created_at
            FROM evaluation_runs
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("load: {e}")))?;

        match row {
            None => Ok(None),
            Some(row) => {
                let mut read_model = EvaluationRunReadModel::try_from(row)?;
                read_model.variant_results = self.load_variant_results(run_id).await?;
                Ok(Some(read_model))
            }
        }
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError> {
        let rows: Vec<RunRow> = sqlx::query_as(
            r#"
            SELECT
                run_id, dataset_id, pipeline_configuration_id, document_id, document_version,
                variants, options, autotune_request,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, created_at
            FROM evaluation_runs
            WHERE document_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("list_for_document: {e}")))?;

        rows.into_iter()
            .map(EvaluationRunReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_for_dataset(
        &self,
        dataset_id: Uuid,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError> {
        let rows: Vec<RunRow> = sqlx::query_as(
            r#"
            SELECT
                run_id, dataset_id, pipeline_configuration_id, document_id, document_version,
                variants, options, autotune_request,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, created_at
            FROM evaluation_runs
            WHERE dataset_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("list_for_dataset: {e}")))?;

        rows.into_iter()
            .map(EvaluationRunReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn load_variant_results(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<EvaluationVariantResultDto>, EvaluationRunRepositoryError> {
        let rows: Vec<VariantResultRow> = sqlx::query_as(
            r#"
            SELECT
                run_id, variant_label, split, variant_config, options,
                recall_mean, recall_std,
                precision_mean, precision_std, iou_mean, iou_std,
                precision_omega_mean, precision_omega_std,
                chunk_set_id, embedding_set_id, selected
            FROM evaluation_variant_results
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("load_variant_results: {e}"))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let trace_rows: Vec<RetrievalTraceRow> = sqlx::query_as(
                r#"
                SELECT
                    question_sequence, retrieved_chunk_ids, scores, recall, precision, iou
                FROM retrieval_traces
                WHERE run_id = $1 AND variant_label = $2 AND split = $3
                ORDER BY question_sequence ASC
                "#,
            )
            .bind(run_id)
            .bind(&row.variant_label)
            .bind(&row.split)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("load_retrieval_traces: {e}"))
            })?;

            let variant_config = serde_json::from_value(row.variant_config).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize variant_config: {e}"))
            })?;
            let options = serde_json::from_value(row.options).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize options: {e}"))
            })?;

            results.push(EvaluationVariantResultDto {
                run_id: row.run_id,
                variant_label: row.variant_label,
                variant_config,
                options,
                split: EvaluationResultSplit::parse(&row.split)
                    .unwrap_or(EvaluationResultSplit::Full),
                recall_mean: row.recall_mean,
                recall_std: row.recall_std,
                precision_mean: row.precision_mean,
                precision_std: row.precision_std,
                iou_mean: row.iou_mean,
                iou_std: row.iou_std,
                precision_omega_mean: row.precision_omega_mean,
                precision_omega_std: row.precision_omega_std,
                chunk_set_id: row.chunk_set_id,
                embedding_set_id: row.embedding_set_id,
                selected: row.selected,
                retrieval_traces: trace_rows
                    .into_iter()
                    .map(|r| {
                        use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
                        RetrievalTraceEntry::try_from(r)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            });
        }

        Ok(results)
    }

    async fn insert_summary(
        &self,
        summary: NewRunSummary,
    ) -> Result<(), EvaluationRunRepositoryError> {
        let created_at = to_offset_datetime(&summary.created_at)
            .map_err(|e| EvaluationRunRepositoryError::Internal(format!("{e}")))?;

        let variants = serde_json::to_value(&summary.variants).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize variants: {e}"))
        })?;
        let options = serde_json::to_value(&summary.options).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize options: {e}"))
        })?;
        let autotune_request = serde_json::to_value(&summary.autotune_request).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize autotune_request: {e}"))
        })?;

        sqlx::query(
            r#"
            INSERT INTO evaluation_runs (
                run_id, dataset_id, pipeline_configuration_id, document_id, document_version,
                variants, options, autotune_request,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9, 0, 0, NULL, $10, $11, $12, $13, $14, NOW())
            ON CONFLICT (run_id) DO NOTHING
            "#,
        )
        .bind(summary.run_id)
        .bind(summary.dataset_id)
        .bind(summary.pipeline_configuration_id)
        .bind(summary.document_id)
        .bind(summary.document_version as i32)
        .bind(&variants)
        .bind(&options)
        .bind(&autotune_request)
        .bind(summary.variants_count as i32)
        .bind(summary.scoring_policy.weights.recall)
        .bind(summary.scoring_policy.weights.iou)
        .bind(summary.scoring_policy.weights.precision)
        .bind(summary.scoring_policy.weights.precision_omega)
        .bind(created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("insert_summary: {e}")))?;

        Ok(())
    }

    async fn record_variant_prepared(
        &self,
        run_id: Uuid,
    ) -> Result<(), EvaluationRunRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_runs SET variants_prepared = variants_prepared + 1, status = 'running', updated_at = NOW() WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("record_variant_prepared: {e}")))?;
        Ok(())
    }

    async fn save_variant_result(
        &self,
        result: EvaluationVariantResultDto,
    ) -> Result<(), EvaluationRunRepositoryError> {
        let variant_config = serde_json::to_value(result.variant_config).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize variant_config: {e}"))
        })?;
        let options = serde_json::to_value(&result.options).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize options: {e}"))
        })?;

        let mut tx = self.pool.begin().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("begin transaction: {e}"))
        })?;

        let inserted: (bool,) = sqlx::query_as(
            r#"
            INSERT INTO evaluation_variant_results (
                run_id, variant_label, split, variant_config, options,
                recall_mean, recall_std,
                precision_mean, precision_std, iou_mean, iou_std,
                precision_omega_mean, precision_omega_std,
                chunk_set_id, embedding_set_id, selected
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (run_id, variant_label, split) DO UPDATE SET
                variant_config = EXCLUDED.variant_config,
                options = EXCLUDED.options,
                recall_mean = EXCLUDED.recall_mean,
                recall_std = EXCLUDED.recall_std,
                precision_mean = EXCLUDED.precision_mean,
                precision_std = EXCLUDED.precision_std,
                iou_mean = EXCLUDED.iou_mean,
                iou_std = EXCLUDED.iou_std,
                precision_omega_mean = EXCLUDED.precision_omega_mean,
                precision_omega_std = EXCLUDED.precision_omega_std,
                selected = EXCLUDED.selected
            RETURNING (xmax = 0) AS is_new
            "#,
        )
        .bind(result.run_id)
        .bind(&result.variant_label)
        .bind(result.split.as_str())
        .bind(&variant_config)
        .bind(&options)
        .bind(result.recall_mean)
        .bind(result.recall_std)
        .bind(result.precision_mean)
        .bind(result.precision_std)
        .bind(result.iou_mean)
        .bind(result.iou_std)
        .bind(result.precision_omega_mean)
        .bind(result.precision_omega_std)
        .bind(result.chunk_set_id)
        .bind(result.embedding_set_id)
        .bind(result.selected)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("save_variant_result: {e}")))?;

        for trace in &result.retrieval_traces {
            let retrieved_chunk_ids =
                serde_json::to_value(&trace.retrieved_chunk_ids).map_err(|e| {
                    EvaluationRunRepositoryError::Internal(format!(
                        "serialize retrieved_chunk_ids: {e}"
                    ))
                })?;
            let scores = serde_json::to_value(&trace.scores).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("serialize scores: {e}"))
            })?;

            sqlx::query(
                r#"
                INSERT INTO retrieval_traces (
                    run_id, variant_label, split, question_sequence,
                    retrieved_chunk_ids, scores, recall, precision, iou
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (run_id, variant_label, split, question_sequence) DO UPDATE SET
                    retrieved_chunk_ids = EXCLUDED.retrieved_chunk_ids,
                    scores = EXCLUDED.scores,
                    recall = EXCLUDED.recall,
                    precision = EXCLUDED.precision,
                    iou = EXCLUDED.iou
                "#,
            )
            .bind(result.run_id)
            .bind(&result.variant_label)
            .bind(result.split.as_str())
            .bind(trace.question_sequence as i32)
            .bind(&retrieved_chunk_ids)
            .bind(&scores)
            .bind(trace.recall)
            .bind(trace.precision)
            .bind(trace.iou)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("save_retrieval_trace: {e}"))
            })?;
        }

        if inserted.0 {
            sqlx::query(
                "UPDATE evaluation_runs SET variants_scored = variants_scored + 1, status = 'running', updated_at = NOW() WHERE run_id = $1",
            )
            .bind(result.run_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| EvaluationRunRepositoryError::Internal(format!("bump variants_scored: {e}")))?;
        }

        tx.commit().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("commit transaction: {e}"))
        })?;

        Ok(())
    }

    async fn mark_completed(&self, run_id: Uuid) -> Result<(), EvaluationRunRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_runs SET status = 'completed', failure_reason = NULL, updated_at = NOW() WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_completed: {e}")))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        run_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationRunRepositoryError> {
        sqlx::query(
            "UPDATE evaluation_runs SET status = 'failed', failure_reason = $2, updated_at = NOW() WHERE run_id = $1",
        )
        .bind(run_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_failed: {e}")))?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct RunRow {
    run_id: Uuid,
    dataset_id: Uuid,
    pipeline_configuration_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    variants: serde_json::Value,
    options: serde_json::Value,
    autotune_request: Option<serde_json::Value>,
    status: String,
    variants_count: i32,
    variants_prepared: i32,
    variants_scored: i32,
    failure_reason: Option<String>,
    scoring_recall_weight: f32,
    scoring_iou_weight: f32,
    scoring_precision_weight: f32,
    scoring_precision_omega_weight: f32,
    created_at: time::OffsetDateTime,
}

impl TryFrom<RunRow> for EvaluationRunReadModel {
    type Error = EvaluationRunRepositoryError;

    fn try_from(row: RunRow) -> Result<Self, Self::Error> {
        let variants: Vec<ChunkingVariant> = serde_json::from_value(row.variants).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("deserialize variants: {e}"))
        })?;
        let options: Vec<EvaluationRunOptions> =
            serde_json::from_value(row.options).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize options: {e}"))
            })?;
        let autotune_request: Option<EvaluationAutotuneRequest> = row
            .autotune_request
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(Self {
            run_id: row.run_id,
            dataset_id: row.dataset_id,
            pipeline_configuration_id: row.pipeline_configuration_id,
            document_id: row.document_id,
            document_version: row.document_version as u32,
            variants,
            options,
            autotune_request,
            status: EvaluationRunStatus::from_parts(
                &row.status,
                row.variants_scored as u32,
                row.failure_reason.clone(),
            )
            .unwrap_or(EvaluationRunStatus::Pending),
            variants_count: row.variants_count as u32,
            variants_prepared: row.variants_prepared as u32,
            variants_scored: row.variants_scored as u32,
            failure_reason: row.failure_reason,
            scoring_policy: ScoringPolicy {
                weights: ScoringWeights {
                    recall: row.scoring_recall_weight,
                    iou: row.scoring_iou_weight,
                    precision: row.scoring_precision_weight,
                    precision_omega: row.scoring_precision_omega_weight,
                },
            },
            created_at: Timestamp::from(row.created_at.format(&Rfc3339).unwrap_or_default()),
            variant_results: Vec::new(),
        })
    }
}

#[derive(sqlx::FromRow)]
struct VariantResultRow {
    run_id: Uuid,
    variant_label: String,
    split: String,
    variant_config: serde_json::Value,
    options: serde_json::Value,
    recall_mean: f32,
    recall_std: f32,
    precision_mean: f32,
    precision_std: f32,
    iou_mean: f32,
    iou_std: f32,
    precision_omega_mean: f32,
    precision_omega_std: f32,
    chunk_set_id: Uuid,
    embedding_set_id: Uuid,
    selected: bool,
}

#[derive(sqlx::FromRow)]
struct RetrievalTraceRow {
    question_sequence: i32,
    retrieved_chunk_ids: serde_json::Value,
    scores: serde_json::Value,
    recall: f32,
    precision: f32,
    iou: f32,
}

impl TryFrom<RetrievalTraceRow>
    for crate::server::domain::evaluation::run::events::RetrievalTraceEntry
{
    type Error = EvaluationRunRepositoryError;

    fn try_from(row: RetrievalTraceRow) -> Result<Self, Self::Error> {
        let retrieved_chunk_ids = serde_json::from_value(row.retrieved_chunk_ids).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("deserialize retrieved_chunk_ids: {e}"))
        })?;
        let scores = serde_json::from_value(row.scores).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("deserialize scores: {e}"))
        })?;
        Ok(Self {
            question_sequence: row.question_sequence as u32,
            retrieved_chunk_ids,
            scores,
            recall: row.recall,
            precision: row.precision,
            iou: row.iou,
        })
    }
}
