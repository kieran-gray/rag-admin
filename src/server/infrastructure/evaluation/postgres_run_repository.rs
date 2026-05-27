use std::collections::{BTreeMap, HashMap, HashSet};

use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::domain::evaluation::run::aggregate::EvaluationRunStatus;
use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
use crate::server::domain::evaluation::run::read_model::{
    BestVariantSnapshot, EvaluationRunReadModel, EvaluationVariantResultRow, NewRunSummary,
};
use crate::server::domain::evaluation::run::repository::{
    ChunkExcerpt, EvaluationRunRepository, EvaluationRunRepositoryError, QuestionResultRow,
    ReferenceExcerpt, RunKindFilter, RunListCursor, RunListPage, RunListQuery,
    RunQuestionResultsLoad, RunStatusFilter, VariantSplitOption,
};
use crate::server::domain::evaluation::run::scoring_policy::{ScoringPolicy, ScoringWeights};
use crate::server::domain::evaluation::value_objects::{
    ChunkingVariant, EvaluationResultSplit, EvaluationRunOptions, OptimizationConfig,
    RunFingerprint,
};
use crate::server::domain::shared::Timestamp;
use crate::server::infrastructure::shared::sql::timestamps::to_offset_datetime;

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
            "
            SELECT
                run_id, dataset_id, index_profile_id, retrieval_profile_id, document_id, document_version,
                variants, options, optimization,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, fingerprint, created_at
            FROM evaluation_runs
            WHERE run_id = $1
            ",
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
        let rows: Vec<RunListViewRow> = sqlx::query_as(
            "
            SELECT
                run_id, dataset_id, document_id, optimization, kind,
                status, failure_reason,
                variant_count, variants_scored,
                best_variant, created_at
            FROM evaluation_run_list_view
            WHERE document_id = $1
            ORDER BY created_at DESC, run_id DESC
            ",
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
        let rows: Vec<RunListViewRow> = sqlx::query_as(
            "
            SELECT
                run_id, dataset_id, document_id, optimization, kind,
                status, failure_reason,
                variant_count, variants_scored,
                best_variant, created_at
            FROM evaluation_run_list_view
            WHERE dataset_id = $1
            ORDER BY created_at DESC, run_id DESC
            ",
        )
        .bind(dataset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("list_for_dataset: {e}")))?;

        rows.into_iter()
            .map(EvaluationRunReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_recent(
        &self,
        limit: u32,
    ) -> Result<Vec<EvaluationRunReadModel>, EvaluationRunRepositoryError> {
        let rows: Vec<RunListViewRow> = sqlx::query_as(
            "
            SELECT
                run_id, dataset_id, document_id, optimization, kind,
                status, failure_reason,
                variant_count, variants_scored,
                best_variant, created_at
            FROM evaluation_run_list_view
            ORDER BY created_at DESC, run_id DESC
            LIMIT $1
            ",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("list_recent: {e}")))?;

        rows.into_iter()
            .map(EvaluationRunReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn list_page(
        &self,
        query: &RunListQuery,
    ) -> Result<RunListPage, EvaluationRunRepositoryError> {
        let limit = query.limit.clamp(1, 200) as i64;
        let fetch_limit = limit + 1;

        let total_matching: i64 = {
            let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                "SELECT COUNT(*)::BIGINT FROM evaluation_run_list_view WHERE 1=1",
            );
            push_view_status_filter(&mut qb, &query.statuses);
            push_view_kind_filter(&mut qb, &query.kinds);
            qb.build_query_scalar::<i64>()
                .fetch_one(&self.pool)
                .await
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("count: {e}")))?
        };

        let total_all: i64 =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*)::BIGINT FROM evaluation_run_list_view")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("count_all: {e}")))?;

        let count_rows = sqlx::query(
            "SELECT status, COUNT(*)::BIGINT AS count FROM evaluation_run_list_view GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("status_counts: {e}")))?;

        let mut status_counts: Vec<(RunStatusFilter, u64)> = Vec::new();
        for row in count_rows {
            let status: String = row
                .try_get("status")
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("status row: {e}")))?;
            let count: i64 = row
                .try_get("count")
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("count row: {e}")))?;
            if let Some(filter) = status_filter_from_str(&status) {
                status_counts.push((filter, count.max(0) as u64));
            }
        }
        status_counts.sort_by_key(|(f, _)| status_filter_order(*f));

        let kind_rows = sqlx::query(
            "SELECT kind, COUNT(*)::BIGINT AS count
             FROM evaluation_run_list_view GROUP BY kind",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("kind_counts: {e}")))?;

        let mut kind_counts: Vec<(RunKindFilter, u64)> = Vec::new();
        for row in kind_rows {
            let kind: String = row
                .try_get("kind")
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("kind row: {e}")))?;
            let count: i64 = row.try_get("count").map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("kind count row: {e}"))
            })?;
            let filter = match kind.as_str() {
                "optimization" => RunKindFilter::Optimization,
                "manual" => RunKindFilter::Manual,
                _ => continue,
            };
            kind_counts.push((filter, count.max(0) as u64));
        }
        kind_counts.sort_by_key(|(f, _)| match f {
            RunKindFilter::Optimization => 0,
            RunKindFilter::Manual => 1,
        });

        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT
                run_id, dataset_id, document_id, optimization, kind,
                status, failure_reason,
                variant_count, variants_scored,
                best_variant, created_at
            FROM evaluation_run_list_view
            WHERE 1=1",
        );
        push_view_status_filter(&mut qb, &query.statuses);
        push_view_kind_filter(&mut qb, &query.kinds);

        if let Some(cursor) = &query.cursor {
            qb.push(" AND (created_at, run_id) < (");
            qb.push_bind(cursor.created_at);
            qb.push(", ");
            qb.push_bind(cursor.run_id);
            qb.push(")");
        }
        qb.push(" ORDER BY created_at DESC, run_id DESC LIMIT ");
        qb.push_bind(fetch_limit);

        let rows: Vec<RunListViewRow> = qb
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| EvaluationRunRepositoryError::Internal(format!("list_page: {e}")))?;

        let mut models: Vec<EvaluationRunReadModel> = rows
            .into_iter()
            .map(EvaluationRunReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = if models.len() as i64 > limit {
            models.pop();
            models
                .last()
                .map(|m| -> Result<RunListCursor, EvaluationRunRepositoryError> {
                    let parsed = OffsetDateTime::parse(&m.created_at.to_string(), &Rfc3339)
                        .map_err(|e| {
                            EvaluationRunRepositoryError::Internal(format!("cursor timestamp: {e}"))
                        })?;
                    Ok(RunListCursor {
                        created_at: parsed,
                        run_id: m.run_id,
                    })
                })
                .transpose()?
        } else {
            None
        };

        Ok(RunListPage {
            items: models,
            next_cursor,
            total_matching: total_matching.max(0) as u64,
            total_all: total_all.max(0) as u64,
            status_counts,
            kind_counts,
        })
    }

    async fn load_variant_results(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<EvaluationVariantResultRow>, EvaluationRunRepositoryError> {
        let rows: Vec<VariantResultRow> = sqlx::query_as(
            "
            SELECT
                run_id, variant_label, split, variant_config, options,
                top_k, min_score_milli,
                recall_mean, recall_std,
                precision_mean, precision_std, iou_mean, iou_std,
                precision_omega_mean, precision_omega_std,
                chunk_set_id, embedding_set_id,
                chunk_count, average_chunk_tokens, average_retrieved_tokens,
                selected,
                recall_ci_low, recall_ci_high,
                precision_ci_low, precision_ci_high,
                iou_ci_low, iou_ci_high,
                precision_omega_ci_low, precision_omega_ci_high,
                composite_ci_low, composite_ci_high,
                judge_score
            FROM evaluation_variant_results
            WHERE run_id = $1
            ORDER BY variant_label, top_k, min_score_milli
            ",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("load_variant_results: {e}"))
        })?;

        let trace_rows: Vec<RetrievalTraceRowFull> = sqlx::query_as(
            "
            SELECT
                variant_label, split, top_k, min_score_milli,
                question_sequence, retrieved_chunk_ids, scores, recall, precision, iou,
                operation, evidence_kind
            FROM retrieval_traces
            WHERE run_id = $1
            ORDER BY variant_label, split, top_k, min_score_milli, question_sequence
            ",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("load_retrieval_traces: {e}"))
        })?;

        let mut traces_by_variant: HashMap<(String, String, i32, i32), Vec<RetrievalTraceRow>> =
            HashMap::new();
        for tr in trace_rows {
            traces_by_variant
                .entry((
                    tr.variant_label.clone(),
                    tr.split.clone(),
                    tr.top_k,
                    tr.min_score_milli,
                ))
                .or_default()
                .push(RetrievalTraceRow {
                    question_sequence: tr.question_sequence,
                    retrieved_chunk_ids: tr.retrieved_chunk_ids,
                    scores: tr.scores,
                    recall: tr.recall,
                    precision: tr.precision,
                    iou: tr.iou,
                    operation: tr.operation,
                    evidence_kind: tr.evidence_kind,
                });
        }

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let key = (
                row.variant_label.clone(),
                row.split.clone(),
                row.top_k,
                row.min_score_milli,
            );
            let trace_rows = traces_by_variant.remove(&key).unwrap_or_default();

            let variant_config = serde_json::from_value(row.variant_config).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize variant_config: {e}"))
            })?;
            let options = serde_json::from_value(row.options).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize options: {e}"))
            })?;

            results.push(EvaluationVariantResultRow {
                run_id: row.run_id,
                variant_label: row.variant_label,
                variant_config,
                options,
                split: EvaluationResultSplit::parse(&row.split).map_err(|e| {
                    EvaluationRunRepositoryError::Internal(format!("parse split: {e}"))
                })?,
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
                chunk_count: row.chunk_count.max(0) as u32,
                average_chunk_tokens: row.average_chunk_tokens.max(0) as u32,
                average_retrieved_tokens: row.average_retrieved_tokens.max(0) as u32,
                selected: row.selected,
                retrieval_traces: trace_rows
                    .into_iter()
                    .map(|r| {
                        use crate::server::domain::evaluation::run::events::RetrievalTraceEntry;
                        RetrievalTraceEntry::try_from(r)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                recall_ci_low: row.recall_ci_low,
                recall_ci_high: row.recall_ci_high,
                precision_ci_low: row.precision_ci_low,
                precision_ci_high: row.precision_ci_high,
                iou_ci_low: row.iou_ci_low,
                iou_ci_high: row.iou_ci_high,
                precision_omega_ci_low: row.precision_omega_ci_low,
                precision_omega_ci_high: row.precision_omega_ci_high,
                composite_ci_low: row.composite_ci_low,
                composite_ci_high: row.composite_ci_high,
                judge_score: row.judge_score,
            });
        }

        Ok(results)
    }

    async fn load_question_results(
        &self,
        run_id: Uuid,
        variant_label: &str,
        split: Option<EvaluationResultSplit>,
    ) -> Result<Option<RunQuestionResultsLoad>, EvaluationRunRepositoryError> {
        let joined_rows: Vec<VariantSplitRowWithDataset> = sqlx::query_as(
            "
            SELECT vr.variant_label, vr.split, vr.top_k, vr.min_score_milli,
                   vr.options, r.dataset_id
            FROM evaluation_variant_results vr
            JOIN evaluation_runs r ON r.run_id = vr.run_id
            WHERE vr.run_id = $1
            ORDER BY vr.variant_label, vr.split, vr.top_k, vr.min_score_milli
            ",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("load_question_results variants: {e}"))
        })?;

        let Some(dataset_id) = joined_rows.first().map(|r| r.dataset_id) else {
            return Ok(None);
        };

        let variant_rows: Vec<VariantSplitRow> = joined_rows
            .into_iter()
            .map(|r| VariantSplitRow {
                variant_label: r.variant_label,
                split: r.split,
                top_k: r.top_k,
                min_score_milli: r.min_score_milli,
                options: r.options,
            })
            .collect();

        let mut variants_by_label: BTreeMap<String, Vec<EvaluationResultSplit>> = BTreeMap::new();
        for row in &variant_rows {
            let s = EvaluationResultSplit::parse(&row.split)
                .map_err(|e| EvaluationRunRepositoryError::Internal(format!("parse split: {e}")))?;
            let entry = variants_by_label
                .entry(row.variant_label.clone())
                .or_default();
            if !entry.contains(&s) {
                entry.push(s);
            }
        }
        let variants_list: Vec<VariantSplitOption> = variants_by_label
            .into_iter()
            .map(|(variant_label, mut splits)| {
                splits.sort();
                VariantSplitOption {
                    variant_label,
                    splits,
                }
            })
            .collect();

        let candidates: Vec<&VariantSplitRow> = variant_rows
            .iter()
            .filter(|r| r.variant_label == variant_label)
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }

        let Some(chosen) = pick_variant_row(&candidates, split) else {
            return Ok(None);
        };
        let chosen_split = EvaluationResultSplit::parse(&chosen.split)
            .map_err(|e| EvaluationRunRepositoryError::Internal(format!("parse split: {e}")))?;
        let options: EvaluationRunOptions = serde_json::from_value(chosen.options.clone())
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize variant options: {e}"))
            })?;

        let trace_rows: Vec<RetrievalTraceRow> = sqlx::query_as(
            "
            SELECT
                question_sequence, retrieved_chunk_ids, scores, recall, precision, iou,
                operation, evidence_kind
            FROM retrieval_traces
            WHERE run_id = $1 AND variant_label = $2 AND split = $3
              AND top_k = $4 AND min_score_milli = $5
            ORDER BY question_sequence ASC
            ",
        )
        .bind(run_id)
        .bind(&chosen.variant_label)
        .bind(&chosen.split)
        .bind(chosen.top_k)
        .bind(chosen.min_score_milli)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("load_question_results traces: {e}"))
        })?;

        let mut question_sequences: Vec<i32> =
            trace_rows.iter().map(|r| r.question_sequence).collect();
        question_sequences.sort_unstable();
        question_sequences.dedup();

        let question_rows: Vec<(i32, String, String, String, String)> = if question_sequences
            .is_empty()
        {
            Vec::new()
        } else {
            sqlx::query_as(
                "SELECT sequence, question, operation, evidence_kind, role FROM evaluation_questions \
                 WHERE dataset_id = $1 AND sequence = ANY($2)",
            )
            .bind(dataset_id)
            .bind(&question_sequences)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!(
                    "load_question_results questions: {e}"
                ))
            })?
        };
        let question_text: HashMap<u32, (String, String, String, String)> = question_rows
            .into_iter()
            .map(|(seq, q, op, ev, role)| (seq as u32, (q, op, ev, role)))
            .collect();

        let reference_rows: Vec<(i32, i32, String, i32, i32)> = if question_sequences.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(
                "
                SELECT question_sequence, sequence, content, char_start, char_end
                FROM evaluation_references
                WHERE dataset_id = $1 AND question_sequence = ANY($2)
                ORDER BY question_sequence, sequence
                ",
            )
            .bind(dataset_id)
            .bind(&question_sequences)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("load_question_results refs: {e}"))
            })?
        };
        let references: Vec<ReferenceExcerpt> = reference_rows
            .into_iter()
            .map(|(qs, seq, content, cs, ce)| ReferenceExcerpt {
                question_sequence: qs as u32,
                sequence: seq as u32,
                content,
                char_start: cs.max(0) as u32,
                char_end: ce.max(0) as u32,
            })
            .collect();

        let mut parsed_traces: Vec<(u32, String, String, f32, f32, f32, Vec<Uuid>, Vec<f32>)> =
            Vec::with_capacity(trace_rows.len());
        let mut chunk_id_set: HashSet<Uuid> = HashSet::new();
        for row in trace_rows {
            let retrieved_chunk_ids: Vec<Uuid> = serde_json::from_value(row.retrieved_chunk_ids)
                .map_err(|e| {
                    EvaluationRunRepositoryError::Internal(format!(
                        "deserialize retrieved_chunk_ids: {e}"
                    ))
                })?;
            let scores: Vec<f32> = serde_json::from_value(row.scores).map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("deserialize scores: {e}"))
            })?;
            for id in &retrieved_chunk_ids {
                chunk_id_set.insert(*id);
            }
            parsed_traces.push((
                row.question_sequence as u32,
                row.operation,
                row.evidence_kind,
                row.recall,
                row.precision,
                row.iou,
                retrieved_chunk_ids,
                scores,
            ));
        }
        let chunk_ids: Vec<Uuid> = chunk_id_set.into_iter().collect();

        let chunk_rows: Vec<(Uuid, i32, String, String, i32, i32)> = if chunk_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as(
                "
                SELECT chunk_id, sequence, heading, text, char_start, char_end
                FROM chunks
                WHERE chunk_id = ANY($1)
                ",
            )
            .bind(&chunk_ids)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("load_question_results chunks: {e}"))
            })?
        };
        let chunks: HashMap<Uuid, ChunkExcerpt> = chunk_rows
            .into_iter()
            .map(|(id, seq, heading, text, cs, ce)| {
                (
                    id,
                    ChunkExcerpt {
                        chunk_id: id,
                        sequence: seq.max(0) as u32,
                        heading,
                        text,
                        char_start: cs.max(0) as u32,
                        char_end: ce.max(0) as u32,
                    },
                )
            })
            .collect();

        let mut questions: Vec<QuestionResultRow> = Vec::with_capacity(parsed_traces.len());
        for (seq, operation, evidence_kind, recall, precision, iou, retrieved_chunk_ids, scores) in
            parsed_traces
        {
            let (question, _, _, role) = question_text.get(&seq).cloned().unwrap_or_else(|| {
                (
                    format!("Q{}", seq + 1),
                    String::new(),
                    String::new(),
                    String::new(),
                )
            });
            questions.push(QuestionResultRow {
                sequence: seq,
                question,
                operation,
                evidence_kind,
                role,
                recall,
                precision,
                iou,
                retrieved_chunk_ids,
                scores,
            });
        }

        Ok(Some(RunQuestionResultsLoad {
            run_id,
            dataset_id,
            variant_label: chosen.variant_label.clone(),
            split: chosen_split,
            options,
            questions,
            references,
            chunks,
            variants: variants_list,
        }))
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
        let optimization = serde_json::to_value(&summary.optimization).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize optimization: {e}"))
        })?;
        let fingerprint = serde_json::to_value(&summary.fingerprint).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("serialize fingerprint: {e}"))
        })?;

        let mut tx = self.pool.begin().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("begin transaction: {e}"))
        })?;

        sqlx::query(
            "
            INSERT INTO evaluation_runs (
                run_id, dataset_id, index_profile_id, retrieval_profile_id,
                document_id, document_version,
                variants, options, optimization,
                status, variants_count, variants_prepared, variants_scored, failure_reason,
                scoring_recall_weight, scoring_iou_weight, scoring_precision_weight,
                scoring_precision_omega_weight, fingerprint, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10, 0, 0, NULL, $11, $12, $13, $14, $15, $16, NOW())
            ON CONFLICT (run_id) DO NOTHING
            ",
        )
        .bind(summary.run_id)
        .bind(summary.dataset_id)
        .bind(summary.index_profile_id)
        .bind(summary.retrieval_profile_id)
        .bind(summary.document_id)
        .bind(summary.document_version as i32)
        .bind(&variants)
        .bind(&options)
        .bind(&optimization)
        .bind(summary.variants_count as i32)
        .bind(summary.scoring_policy.weights.recall)
        .bind(summary.scoring_policy.weights.iou)
        .bind(summary.scoring_policy.weights.precision)
        .bind(summary.scoring_policy.weights.precision_omega)
        .bind(&fingerprint)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("insert_summary: {e}")))?;

        let kind = if summary.optimization.is_some() {
            "optimization"
        } else {
            "manual"
        };
        sqlx::query(
            "
            INSERT INTO evaluation_run_list_view (
                run_id, dataset_id, document_id, optimization, kind, status, failure_reason,
                variant_count, variants_scored, best_variant, created_at
            )
            VALUES ($1, $2, $3, $4, $5, 'pending', NULL, $6, 0, NULL, $7)
            ON CONFLICT (run_id) DO NOTHING
            ",
        )
        .bind(summary.run_id)
        .bind(summary.dataset_id)
        .bind(summary.document_id)
        .bind(&optimization)
        .bind(kind)
        .bind(summary.variants_count as i32)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("insert_summary view: {e}")))?;

        tx.commit().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("commit insert_summary: {e}"))
        })?;

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
        result: EvaluationVariantResultRow,
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
            "
            INSERT INTO evaluation_variant_results (
                run_id, variant_label, split, variant_config, options,
                top_k, min_score_milli,
                recall_mean, recall_std,
                precision_mean, precision_std, iou_mean, iou_std,
                precision_omega_mean, precision_omega_std,
                chunk_set_id, embedding_set_id,
                chunk_count, average_chunk_tokens, average_retrieved_tokens,
                selected,
                recall_ci_low, recall_ci_high,
                precision_ci_low, precision_ci_high,
                iou_ci_low, iou_ci_high,
                precision_omega_ci_low, precision_omega_ci_high,
                composite_ci_low, composite_ci_high,
                judge_score
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21,
                    $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32)
            ON CONFLICT (run_id, variant_label, split, top_k, min_score_milli) DO UPDATE SET
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
                chunk_count = EXCLUDED.chunk_count,
                average_chunk_tokens = EXCLUDED.average_chunk_tokens,
                average_retrieved_tokens = EXCLUDED.average_retrieved_tokens,
                selected = EXCLUDED.selected,
                recall_ci_low = EXCLUDED.recall_ci_low,
                recall_ci_high = EXCLUDED.recall_ci_high,
                precision_ci_low = EXCLUDED.precision_ci_low,
                precision_ci_high = EXCLUDED.precision_ci_high,
                iou_ci_low = EXCLUDED.iou_ci_low,
                iou_ci_high = EXCLUDED.iou_ci_high,
                precision_omega_ci_low = EXCLUDED.precision_omega_ci_low,
                precision_omega_ci_high = EXCLUDED.precision_omega_ci_high,
                composite_ci_low = EXCLUDED.composite_ci_low,
                composite_ci_high = EXCLUDED.composite_ci_high,
                judge_score = EXCLUDED.judge_score
            RETURNING (xmax = 0) AS is_new
            ",
        )
        .bind(result.run_id)
        .bind(&result.variant_label)
        .bind(result.split.as_str())
        .bind(&variant_config)
        .bind(&options)
        .bind(result.options.top_k as i32)
        .bind(result.options.min_score_milli as i32)
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
        .bind(result.chunk_count as i32)
        .bind(result.average_chunk_tokens as i32)
        .bind(result.average_retrieved_tokens as i32)
        .bind(result.selected)
        .bind(result.recall_ci_low)
        .bind(result.recall_ci_high)
        .bind(result.precision_ci_low)
        .bind(result.precision_ci_high)
        .bind(result.iou_ci_low)
        .bind(result.iou_ci_high)
        .bind(result.precision_omega_ci_low)
        .bind(result.precision_omega_ci_high)
        .bind(result.composite_ci_low)
        .bind(result.composite_ci_high)
        .bind(result.judge_score)
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
                "
                INSERT INTO retrieval_traces (
                    run_id, variant_label, split, top_k, min_score_milli, question_sequence,
                    retrieved_chunk_ids, scores, recall, precision, iou, operation, evidence_kind
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                ON CONFLICT (run_id, variant_label, split, top_k, min_score_milli, question_sequence) DO UPDATE SET
                    retrieved_chunk_ids = EXCLUDED.retrieved_chunk_ids,
                    scores = EXCLUDED.scores,
                    recall = EXCLUDED.recall,
                    precision = EXCLUDED.precision,
                    iou = EXCLUDED.iou,
                    operation = EXCLUDED.operation,
                    evidence_kind = EXCLUDED.evidence_kind
                ",
            )
            .bind(result.run_id)
            .bind(&result.variant_label)
            .bind(result.split.as_str())
            .bind(result.options.top_k as i32)
            .bind(result.options.min_score_milli as i32)
            .bind(trace.question_sequence as i32)
            .bind(&retrieved_chunk_ids)
            .bind(&scores)
            .bind(trace.recall)
            .bind(trace.precision)
            .bind(trace.iou)
            .bind(&trace.operation)
            .bind(&trace.evidence_kind)
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

        sqlx::query(BEST_VARIANT_VIEW_UPDATE_SQL)
            .bind(result.run_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                EvaluationRunRepositoryError::Internal(format!("recompute view best_variant: {e}"))
            })?;

        tx.commit().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("commit transaction: {e}"))
        })?;

        Ok(())
    }

    async fn mark_completed(&self, run_id: Uuid) -> Result<(), EvaluationRunRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("begin transaction: {e}"))
        })?;
        sqlx::query(
            "UPDATE evaluation_runs SET status = 'completed', failure_reason = NULL, updated_at = NOW() WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_completed: {e}")))?;
        sqlx::query(
            "UPDATE evaluation_run_list_view SET status = 'completed', failure_reason = NULL WHERE run_id = $1",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_completed view: {e}")))?;
        tx.commit().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("commit mark_completed: {e}"))
        })?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        run_id: Uuid,
        reason: String,
    ) -> Result<(), EvaluationRunRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("begin transaction: {e}"))
        })?;
        sqlx::query(
            "UPDATE evaluation_runs SET status = 'failed', failure_reason = $2, updated_at = NOW() WHERE run_id = $1",
        )
        .bind(run_id)
        .bind(&reason)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_failed: {e}")))?;
        sqlx::query(
            "UPDATE evaluation_run_list_view SET status = 'failed', failure_reason = $2 WHERE run_id = $1",
        )
        .bind(run_id)
        .bind(&reason)
        .execute(&mut *tx)
        .await
        .map_err(|e| EvaluationRunRepositoryError::Internal(format!("mark_failed view: {e}")))?;
        tx.commit().await.map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("commit mark_failed: {e}"))
        })?;
        Ok(())
    }
}

const BEST_VARIANT_VIEW_UPDATE_SQL: &str = "
WITH r AS (
    SELECT scoring_recall_weight AS w_r,
           scoring_iou_weight AS w_i,
           scoring_precision_weight AS w_p,
           scoring_precision_omega_weight AS w_po
    FROM evaluation_runs WHERE run_id = $1
),
scored AS (
    SELECT v.*, (
        v.recall_mean * r.w_r +
        v.iou_mean * r.w_i +
        v.precision_mean * r.w_p +
        v.precision_omega_mean * r.w_po
    ) AS score
    FROM evaluation_variant_results v, r
    WHERE v.run_id = $1
),
best AS (
    SELECT * FROM scored ORDER BY score DESC LIMIT 1
)
UPDATE evaluation_run_list_view lv
SET variants_scored = (
        SELECT COUNT(*)::INT FROM evaluation_variant_results WHERE run_id = $1
    ),
    status = CASE
        WHEN lv.status IN ('completed', 'failed') THEN lv.status
        ELSE 'running'
    END,
    best_variant = (
        SELECT jsonb_build_object(
            'label', best.variant_label,
            'config', best.variant_config,
            'options', best.options,
            'score', best.score,
            'metrics', jsonb_build_object(
                'recall_mean', best.recall_mean,
                'recall_std', best.recall_std,
                'precision_mean', best.precision_mean,
                'precision_std', best.precision_std,
                'iou_mean', best.iou_mean,
                'iou_std', best.iou_std,
                'precision_omega_mean', best.precision_omega_mean,
                'precision_omega_std', best.precision_omega_std,
                'recall_ci_low', best.recall_ci_low,
                'recall_ci_high', best.recall_ci_high,
                'precision_ci_low', best.precision_ci_low,
                'precision_ci_high', best.precision_ci_high,
                'iou_ci_low', best.iou_ci_low,
                'iou_ci_high', best.iou_ci_high,
                'precision_omega_ci_low', best.precision_omega_ci_low,
                'precision_omega_ci_high', best.precision_omega_ci_high,
                'composite_ci_low', best.composite_ci_low,
                'composite_ci_high', best.composite_ci_high,
                'chunk_count', best.chunk_count,
                'average_chunk_tokens', best.average_chunk_tokens,
                'average_retrieved_tokens', best.average_retrieved_tokens,
                'judge_score', best.judge_score
            )
        )
        FROM best
    )
WHERE lv.run_id = $1
";

fn push_view_status_filter(qb: &mut QueryBuilder<'_, Postgres>, statuses: &[RunStatusFilter]) {
    if statuses.is_empty() {
        return;
    }
    qb.push(" AND status IN (");
    let mut sep = qb.separated(", ");
    for status in statuses {
        sep.push_bind(status_filter_as_str(*status));
    }
    sep.push_unseparated(")");
}

fn push_view_kind_filter(qb: &mut QueryBuilder<'_, Postgres>, kinds: &[RunKindFilter]) {
    if kinds.is_empty() {
        return;
    }
    qb.push(" AND kind IN (");
    let mut sep = qb.separated(", ");
    for kind in kinds {
        let label = match kind {
            RunKindFilter::Optimization => "optimization",
            RunKindFilter::Manual => "manual",
        };
        sep.push_bind(label);
    }
    sep.push_unseparated(")");
}

fn status_filter_as_str(status: RunStatusFilter) -> &'static str {
    match status {
        RunStatusFilter::Completed => "completed",
        RunStatusFilter::Running => "running",
        RunStatusFilter::Failed => "failed",
        RunStatusFilter::Pending => "pending",
    }
}

fn status_filter_from_str(status: &str) -> Option<RunStatusFilter> {
    match status {
        "completed" => Some(RunStatusFilter::Completed),
        "running" => Some(RunStatusFilter::Running),
        "failed" => Some(RunStatusFilter::Failed),
        "pending" => Some(RunStatusFilter::Pending),
        _ => None,
    }
}

fn status_filter_order(status: RunStatusFilter) -> u8 {
    match status {
        RunStatusFilter::Running => 0,
        RunStatusFilter::Pending => 1,
        RunStatusFilter::Completed => 2,
        RunStatusFilter::Failed => 3,
    }
}

#[derive(sqlx::FromRow)]
struct RunRow {
    run_id: Uuid,
    dataset_id: Uuid,
    index_profile_id: Uuid,
    retrieval_profile_id: Option<Uuid>,
    document_id: Uuid,
    document_version: i32,
    variants: serde_json::Value,
    options: serde_json::Value,
    optimization: Option<serde_json::Value>,
    status: String,
    variants_count: i32,
    variants_prepared: i32,
    variants_scored: i32,
    failure_reason: Option<String>,
    scoring_recall_weight: f32,
    scoring_iou_weight: f32,
    scoring_precision_weight: f32,
    scoring_precision_omega_weight: f32,
    fingerprint: serde_json::Value,
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
        let optimization: Option<OptimizationConfig> = row
            .optimization
            .and_then(|v| serde_json::from_value(v).ok());
        let fingerprint = serde_json::from_value(row.fingerprint).map_err(|e| {
            EvaluationRunRepositoryError::Internal(format!("deserialize fingerprint: {e}"))
        })?;

        Ok(Self {
            run_id: row.run_id,
            dataset_id: row.dataset_id,
            index_profile_id: row.index_profile_id,
            retrieval_profile_id: row.retrieval_profile_id,
            document_id: row.document_id,
            document_version: row.document_version as u32,
            variants,
            options,
            optimization,
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
            fingerprint,
            created_at: Timestamp::from(row.created_at.format(&Rfc3339).unwrap_or_default()),
            variant_results: Vec::new(),
            best_variant: None,
        })
    }
}

#[derive(sqlx::FromRow)]
struct RunListViewRow {
    run_id: Uuid,
    dataset_id: Uuid,
    document_id: Uuid,
    optimization: Option<serde_json::Value>,
    kind: String,
    status: String,
    failure_reason: Option<String>,
    variant_count: i32,
    variants_scored: i32,
    best_variant: Option<serde_json::Value>,
    created_at: time::OffsetDateTime,
}

impl TryFrom<RunListViewRow> for EvaluationRunReadModel {
    type Error = EvaluationRunRepositoryError;

    fn try_from(row: RunListViewRow) -> Result<Self, Self::Error> {
        let _ = row.kind;
        let optimization: Option<OptimizationConfig> = row
            .optimization
            .and_then(|v| serde_json::from_value(v).ok());
        let status = EvaluationRunStatus::from_parts(
            &row.status,
            row.variants_scored.max(0) as u32,
            row.failure_reason.clone(),
        )
        .unwrap_or(EvaluationRunStatus::Pending);
        let best_variant: Option<BestVariantSnapshot> = row
            .best_variant
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(Self {
            run_id: row.run_id,
            dataset_id: row.dataset_id,
            index_profile_id: Uuid::nil(),
            retrieval_profile_id: None,
            document_id: row.document_id,
            document_version: 0,
            variants: Vec::new(),
            options: Vec::new(),
            optimization,
            status,
            variants_count: row.variant_count.max(0) as u32,
            variants_prepared: 0,
            variants_scored: row.variants_scored.max(0) as u32,
            failure_reason: row.failure_reason,
            scoring_policy: ScoringPolicy::default(),
            fingerprint: RunFingerprint {
                document_content_hash: String::new(),
                dataset_content_hash: String::new(),
                embedding_model_snapshot: serde_json::Value::Null,
            },
            created_at: Timestamp::from(row.created_at.format(&Rfc3339).unwrap_or_default()),
            variant_results: Vec::new(),
            best_variant,
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
    top_k: i32,
    min_score_milli: i32,
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
    chunk_count: i32,
    average_chunk_tokens: i32,
    average_retrieved_tokens: i32,
    selected: bool,
    recall_ci_low: f32,
    recall_ci_high: f32,
    precision_ci_low: f32,
    precision_ci_high: f32,
    iou_ci_low: f32,
    iou_ci_high: f32,
    precision_omega_ci_low: f32,
    precision_omega_ci_high: f32,
    composite_ci_low: f32,
    composite_ci_high: f32,
    judge_score: Option<f32>,
}

#[derive(sqlx::FromRow)]
struct VariantSplitRow {
    variant_label: String,
    split: String,
    top_k: i32,
    min_score_milli: i32,
    options: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct VariantSplitRowWithDataset {
    variant_label: String,
    split: String,
    top_k: i32,
    min_score_milli: i32,
    options: serde_json::Value,
    dataset_id: Uuid,
}

fn pick_variant_row<'a>(
    candidates: &'a [&'a VariantSplitRow],
    requested: Option<EvaluationResultSplit>,
) -> Option<&'a VariantSplitRow> {
    if let Some(want) = requested {
        if let Some(row) = candidates
            .iter()
            .find(|r| EvaluationResultSplit::parse(&r.split).ok() == Some(want))
        {
            return Some(row);
        }
    }
    const PREF: [EvaluationResultSplit; 4] = [
        EvaluationResultSplit::Holdout,
        EvaluationResultSplit::Validation,
        EvaluationResultSplit::Full,
        EvaluationResultSplit::Tuning,
    ];
    for pref in PREF {
        if let Some(row) = candidates
            .iter()
            .find(|r| EvaluationResultSplit::parse(&r.split).ok() == Some(pref))
        {
            return Some(row);
        }
    }
    candidates.first().copied()
}

#[derive(sqlx::FromRow)]
struct RetrievalTraceRow {
    question_sequence: i32,
    retrieved_chunk_ids: serde_json::Value,
    scores: serde_json::Value,
    recall: f32,
    precision: f32,
    iou: f32,
    operation: String,
    evidence_kind: String,
}

#[derive(sqlx::FromRow)]
struct RetrievalTraceRowFull {
    variant_label: String,
    split: String,
    top_k: i32,
    min_score_milli: i32,
    question_sequence: i32,
    retrieved_chunk_ids: serde_json::Value,
    scores: serde_json::Value,
    recall: f32,
    precision: f32,
    iou: f32,
    operation: String,
    evidence_kind: String,
}

impl TryFrom<RetrievalTraceRow> for RetrievalTraceEntry {
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
            operation: row.operation,
            evidence_kind: row.evidence_kind,
        })
    }
}
