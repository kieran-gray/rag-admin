use std::fmt::Display;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::map::aggregate::{section_count_for, MapStatus};
use crate::server::domain::comprehension::map::events::MapBuildRequested;
use crate::server::domain::comprehension::map::read_model::{
    DocumentMapDetail, DocumentMapReadModel,
};
use crate::server::domain::comprehension::map::repository::{
    DocumentMapRepository, DocumentMapRepositoryError,
};
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::comprehension::thread::Thread;

pub struct PostgresDocumentMapRepository {
    pool: PgPool,
}

impl PostgresDocumentMapRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn fetch_row(
        &self,
        map_id: Uuid,
    ) -> Result<Option<DocumentMapRow>, DocumentMapRepositoryError> {
        let row = sqlx::query_as::<_, DocumentMapRow>(SELECT_BASE)
            .bind(map_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("fetch_row: {e}")))?;
        Ok(row)
    }

    async fn recompute_status(&self, map_id: Uuid) -> Result<(), DocumentMapRepositoryError> {
        let Some(row) = self.fetch_row(map_id).await? else {
            return Ok(());
        };
        if row.status == "failed" || row.status == "ready" {
            return Ok(());
        }

        let next = derive_status(&row);
        let (status_str, _) = status_to_db(&next);
        sqlx::query("UPDATE document_maps SET status = $2, updated_at = NOW() WHERE map_id = $1")
            .bind(map_id)
            .bind(status_str)
            .execute(&self.pool)
            .await
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("recompute_status: {e}")))?;
        Ok(())
    }
}

fn derive_status(row: &DocumentMapRow) -> MapStatus {
    if row
        .suggested_roles_json()
        .as_array()
        .is_none_or(Vec::is_empty)
        && row.extracted_chunks.is_empty()
        && row.synthesized_sections.is_empty()
    {
        return MapStatus::Pending;
    }

    let chunk_count = row.chunk_count as u32;
    let section_size = row.section_size as u32;

    let extracted = row.extracted_chunks.len() as u32;
    if extracted < chunk_count {
        if row
            .suggested_roles_json()
            .as_array()
            .is_some_and(|a| !a.is_empty())
        {
            return MapStatus::ExtractingObservations {
                remaining_chunks: chunk_count.saturating_sub(extracted),
            };
        }
        return MapStatus::TypingRoles;
    }

    let sections = section_count_for(chunk_count, section_size);
    let synthesized = row.synthesized_sections.len() as u32;
    if synthesized < sections {
        return MapStatus::SynthesizingThreads {
            remaining_sections: sections.saturating_sub(synthesized),
        };
    }

    MapStatus::SynthesizingInsights
}

fn status_to_db(status: &MapStatus) -> (String, Option<String>) {
    match status {
        MapStatus::Pending => ("pending".into(), None),
        MapStatus::TypingRoles => ("typing_roles".into(), None),
        MapStatus::ExtractingObservations { .. } => ("extracting_observations".into(), None),
        MapStatus::SynthesizingThreads { .. } => ("synthesizing_threads".into(), None),
        MapStatus::SynthesizingInsights => ("synthesizing_insights".into(), None),
        MapStatus::Ready => ("ready".into(), None),
        MapStatus::Failed { reason } => ("failed".into(), Some(reason.clone())),
    }
}

const SELECT_BASE: &str = "SELECT map_id, document_id, document_version, content_hash, \
chunk_set_id, chunk_count, section_size, status, failure_reason, generation_model_id, \
suggested_roles, carried_thread_summary, extracted_chunks, synthesized_sections \
FROM document_maps WHERE map_id = $1";

#[derive(sqlx::FromRow)]
struct DocumentMapRow {
    map_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    content_hash: String,
    chunk_set_id: Uuid,
    chunk_count: i32,
    section_size: i32,
    status: String,
    failure_reason: Option<String>,
    generation_model_id: Uuid,
    suggested_roles: JsonValue,
    #[allow(dead_code)]
    carried_thread_summary: String,
    extracted_chunks: Vec<i32>,
    synthesized_sections: Vec<i32>,
}

impl DocumentMapRow {
    fn suggested_roles_json(&self) -> &JsonValue {
        &self.suggested_roles
    }
}

impl DocumentMapRow {
    fn into_read_model(self) -> Result<DocumentMapReadModel, DocumentMapRepositoryError> {
        let roles: Vec<SuggestedRole> = serde_json::from_value(self.suggested_roles)
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("decode roles: {e}")))?;
        Ok(DocumentMapReadModel {
            map_id: self.map_id,
            document_id: self.document_id,
            document_version: self.document_version as u32,
            content_hash: self.content_hash,
            chunk_set_id: self.chunk_set_id,
            chunk_count: self.chunk_count as u32,
            section_size: self.section_size as u32,
            status: self.status,
            failure_reason: self.failure_reason,
            generation_model_id: self.generation_model_id,
            suggested_roles: roles,
            observations_extracted: self.extracted_chunks.len() as u32,
            threads_synthesized: self.synthesized_sections.len() as u32,
            insights_synthesized: 0,
        })
    }
}

#[async_trait]
impl DocumentMapRepository for PostgresDocumentMapRepository {
    async fn load_summary(
        &self,
        map_id: Uuid,
    ) -> Result<Option<DocumentMapReadModel>, DocumentMapRepositoryError> {
        let Some(row) = self.fetch_row(map_id).await? else {
            return Ok(None);
        };
        let insights_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM map_insights WHERE map_id = $1")
                .bind(map_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    DocumentMapRepositoryError::Internal(format!("count insights: {e}"))
                })?;
        let mut read = row.into_read_model()?;
        read.insights_synthesized = insights_count.max(0) as u32;
        Ok(Some(read))
    }

    async fn find_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
    ) -> Result<Option<DocumentMapReadModel>, DocumentMapRepositoryError> {
        let map_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT map_id FROM document_maps WHERE document_id = $1 AND document_version = $2",
        )
        .bind(document_id)
        .bind(document_version as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("find_for_document: {e}")))?;
        if let Some(id) = map_id {
            self.load_summary(id).await
        } else {
            Ok(None)
        }
    }

    async fn load_detail(
        &self,
        map_id: Uuid,
    ) -> Result<Option<DocumentMapDetail>, DocumentMapRepositoryError> {
        let Some(summary) = self.load_summary(map_id).await? else {
            return Ok(None);
        };
        let carried = self.carried_summary(map_id).await?;
        let observation_rows = sqlx::query(
            "SELECT observation_id, chunk_sequence, kind, summary, spans
             FROM map_observations WHERE map_id = $1 ORDER BY chunk_sequence, observation_id",
        )
        .bind(map_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("load_detail obs: {e}")))?;
        let observations: Vec<Observation> = observation_rows
            .into_iter()
            .map(|r| {
                Ok::<Observation, DocumentMapRepositoryError>(Observation {
                    observation_id: r.try_get("observation_id").map_err(internal)?,
                    chunk_sequence: r.try_get::<i32, _>("chunk_sequence").map_err(internal)? as u32,
                    kind: r.try_get::<String, _>("kind").map_err(internal)?,
                    summary: r.try_get::<String, _>("summary").map_err(internal)?,
                    spans: serde_json::from_value(r.try_get("spans").map_err(internal)?).map_err(
                        |e| DocumentMapRepositoryError::Internal(format!("decode spans: {e}")),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let thread_rows = sqlx::query(
            "SELECT thread_id, section_sequence, kind, summary, evidence, spans
             FROM map_threads WHERE map_id = $1 ORDER BY section_sequence, thread_id",
        )
        .bind(map_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("load_detail threads: {e}")))?;
        let threads: Vec<Thread> = thread_rows
            .into_iter()
            .map(|r| {
                Ok::<Thread, DocumentMapRepositoryError>(Thread {
                    thread_id: r.try_get("thread_id").map_err(internal)?,
                    section_sequence: r.try_get::<i32, _>("section_sequence").map_err(internal)?
                        as u32,
                    kind: r.try_get::<String, _>("kind").map_err(internal)?,
                    summary: r.try_get::<String, _>("summary").map_err(internal)?,
                    evidence: serde_json::from_value(r.try_get("evidence").map_err(internal)?)
                        .map_err(|e| {
                            DocumentMapRepositoryError::Internal(format!("decode evidence: {e}"))
                        })?,
                    spans: serde_json::from_value(r.try_get("spans").map_err(internal)?).map_err(
                        |e| DocumentMapRepositoryError::Internal(format!("decode spans: {e}")),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let insight_rows = sqlx::query(
            "SELECT insight_id, kind, summary, evidence, spans
             FROM map_insights WHERE map_id = $1 ORDER BY insight_id",
        )
        .bind(map_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("load_detail insights: {e}")))?;
        let insights: Vec<Insight> = insight_rows
            .into_iter()
            .map(|r| {
                Ok::<Insight, DocumentMapRepositoryError>(Insight {
                    insight_id: r.try_get("insight_id").map_err(internal)?,
                    kind: r.try_get::<String, _>("kind").map_err(internal)?,
                    summary: r.try_get::<String, _>("summary").map_err(internal)?,
                    evidence: serde_json::from_value(r.try_get("evidence").map_err(internal)?)
                        .map_err(|e| {
                            DocumentMapRepositoryError::Internal(format!("decode evidence: {e}"))
                        })?,
                    spans: serde_json::from_value(r.try_get("spans").map_err(internal)?).map_err(
                        |e| DocumentMapRepositoryError::Internal(format!("decode spans: {e}")),
                    )?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(DocumentMapDetail {
            read_model: summary,
            observations,
            threads,
            insights,
            carried_summary: carried,
        }))
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentMapReadModel>, DocumentMapRepositoryError> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT map_id FROM document_maps WHERE document_id = $1 ORDER BY document_version DESC",
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("list_for_document: {e}")))?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(r) = self.load_summary(id).await? {
                out.push(r);
            }
        }
        Ok(out)
    }

    async fn load_observations_for_section(
        &self,
        map_id: Uuid,
        from_chunk_sequence: u32,
        to_chunk_sequence_inclusive: u32,
    ) -> Result<Vec<Observation>, DocumentMapRepositoryError> {
        let rows = sqlx::query(
            "SELECT observation_id, chunk_sequence, kind, summary, spans
             FROM map_observations
             WHERE map_id = $1
               AND chunk_sequence BETWEEN $2 AND $3
             ORDER BY chunk_sequence, observation_id",
        )
        .bind(map_id)
        .bind(from_chunk_sequence as i32)
        .bind(to_chunk_sequence_inclusive as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("load_observations_for_section: {e}"))
        })?;
        rows.into_iter()
            .map(|r| {
                Ok::<Observation, DocumentMapRepositoryError>(Observation {
                    observation_id: r.try_get("observation_id").map_err(internal)?,
                    chunk_sequence: r.try_get::<i32, _>("chunk_sequence").map_err(internal)? as u32,
                    kind: r.try_get::<String, _>("kind").map_err(internal)?,
                    summary: r.try_get::<String, _>("summary").map_err(internal)?,
                    spans: serde_json::from_value(r.try_get("spans").map_err(internal)?).map_err(
                        |e| DocumentMapRepositoryError::Internal(format!("decode spans: {e}")),
                    )?,
                })
            })
            .collect()
    }

    async fn carried_summary(&self, map_id: Uuid) -> Result<String, DocumentMapRepositoryError> {
        let summary: Option<String> = sqlx::query_scalar(
            "SELECT carried_thread_summary FROM document_maps WHERE map_id = $1",
        )
        .bind(map_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("carried_summary: {e}")))?;
        Ok(summary.unwrap_or_default())
    }

    async fn project_requested(
        &self,
        event: &MapBuildRequested,
    ) -> Result<(), DocumentMapRepositoryError> {
        sqlx::query(
            "INSERT INTO document_maps (
                map_id, document_id, document_version, content_hash,
                chunk_set_id, chunk_count, section_size, generation_model_id, status
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending')
             ON CONFLICT (map_id) DO NOTHING",
        )
        .bind(event.map_id)
        .bind(event.document_id)
        .bind(event.document_version as i32)
        .bind(&event.content_hash)
        .bind(event.chunk_set_id)
        .bind(event.chunk_count as i32)
        .bind(event.section_size as i32)
        .bind(event.generation_model_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("project_requested: {e}")))?;
        Ok(())
    }

    async fn project_roles(
        &self,
        map_id: Uuid,
        roles: &[SuggestedRole],
    ) -> Result<(), DocumentMapRepositoryError> {
        let value = serde_json::to_value(roles)
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("encode roles: {e}")))?;
        sqlx::query(
            "UPDATE document_maps SET suggested_roles = $2, updated_at = NOW()
             WHERE map_id = $1",
        )
        .bind(map_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("project_roles: {e}")))?;
        self.recompute_status(map_id).await?;
        Ok(())
    }

    async fn project_observations(
        &self,
        map_id: Uuid,
        chunk_sequence: u32,
        observations: &[Observation],
    ) -> Result<(), DocumentMapRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("project_observations begin: {e}"))
        })?;

        for obs in observations {
            let spans = serde_json::to_value(&obs.spans)
                .map_err(|e| DocumentMapRepositoryError::Internal(format!("encode spans: {e}")))?;
            sqlx::query(
                "INSERT INTO map_observations (map_id, observation_id, chunk_sequence, kind, summary, spans)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (map_id, observation_id) DO NOTHING",
            )
            .bind(map_id)
            .bind(obs.observation_id)
            .bind(obs.chunk_sequence as i32)
            .bind(&obs.kind)
            .bind(&obs.summary)
            .bind(spans)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                DocumentMapRepositoryError::Internal(format!("insert observation: {e}"))
            })?;
        }

        sqlx::query(
            "UPDATE document_maps
             SET extracted_chunks = array_append(extracted_chunks, $2),
                 updated_at = NOW()
             WHERE map_id = $1 AND NOT ($2 = ANY(extracted_chunks))",
        )
        .bind(map_id)
        .bind(chunk_sequence as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("append extracted_chunks: {e}"))
        })?;

        tx.commit().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("commit project_observations: {e}"))
        })?;
        self.recompute_status(map_id).await?;
        Ok(())
    }

    async fn project_chunk_extraction_failure(
        &self,
        map_id: Uuid,
        chunk_sequence: u32,
    ) -> Result<(), DocumentMapRepositoryError> {
        sqlx::query(
            "UPDATE document_maps
             SET extracted_chunks = array_append(extracted_chunks, $2),
                 updated_at = NOW()
             WHERE map_id = $1 AND NOT ($2 = ANY(extracted_chunks))",
        )
        .bind(map_id)
        .bind(chunk_sequence as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("project_chunk_extraction_failure: {e}"))
        })?;
        self.recompute_status(map_id).await?;
        Ok(())
    }

    async fn project_threads(
        &self,
        map_id: Uuid,
        section_sequence: u32,
        carried_summary: &str,
        threads: &[Thread],
    ) -> Result<(), DocumentMapRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("project_threads begin: {e}"))
        })?;

        for thread in threads {
            let evidence = serde_json::to_value(&thread.evidence).map_err(|e| {
                DocumentMapRepositoryError::Internal(format!("encode evidence: {e}"))
            })?;
            let spans = serde_json::to_value(&thread.spans)
                .map_err(|e| DocumentMapRepositoryError::Internal(format!("encode spans: {e}")))?;
            sqlx::query(
                "INSERT INTO map_threads (map_id, thread_id, section_sequence, kind, summary, evidence, spans)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (map_id, thread_id) DO NOTHING",
            )
            .bind(map_id)
            .bind(thread.thread_id)
            .bind(thread.section_sequence as i32)
            .bind(&thread.kind)
            .bind(&thread.summary)
            .bind(evidence)
            .bind(spans)
            .execute(&mut *tx)
            .await
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("insert thread: {e}")))?;
        }

        sqlx::query(
            "UPDATE document_maps
             SET synthesized_sections = array_append(synthesized_sections, $2),
                 carried_thread_summary = $3,
                 updated_at = NOW()
             WHERE map_id = $1 AND NOT ($2 = ANY(synthesized_sections))",
        )
        .bind(map_id)
        .bind(section_sequence as i32)
        .bind(carried_summary)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("append synthesized_sections: {e}"))
        })?;

        tx.commit().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("commit project_threads: {e}"))
        })?;
        self.recompute_status(map_id).await?;
        Ok(())
    }

    async fn project_section_synthesis_failure(
        &self,
        map_id: Uuid,
        section_sequence: u32,
    ) -> Result<(), DocumentMapRepositoryError> {
        sqlx::query(
            "UPDATE document_maps
             SET synthesized_sections = array_append(synthesized_sections, $2),
                 updated_at = NOW()
             WHERE map_id = $1 AND NOT ($2 = ANY(synthesized_sections))",
        )
        .bind(map_id)
        .bind(section_sequence as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("project_section_synthesis_failure: {e}"))
        })?;
        self.recompute_status(map_id).await?;
        Ok(())
    }

    async fn project_insights(
        &self,
        map_id: Uuid,
        insights: &[Insight],
    ) -> Result<(), DocumentMapRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("project_insights begin: {e}"))
        })?;
        for insight in insights {
            let evidence = serde_json::to_value(&insight.evidence).map_err(|e| {
                DocumentMapRepositoryError::Internal(format!("encode evidence: {e}"))
            })?;
            let spans = serde_json::to_value(&insight.spans)
                .map_err(|e| DocumentMapRepositoryError::Internal(format!("encode spans: {e}")))?;
            sqlx::query(
                "INSERT INTO map_insights (map_id, insight_id, kind, summary, evidence, spans)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (map_id, insight_id) DO NOTHING",
            )
            .bind(map_id)
            .bind(insight.insight_id)
            .bind(&insight.kind)
            .bind(&insight.summary)
            .bind(evidence)
            .bind(spans)
            .execute(&mut *tx)
            .await
            .map_err(|e| DocumentMapRepositoryError::Internal(format!("insert insight: {e}")))?;
        }
        sqlx::query(
            "UPDATE document_maps SET status = 'ready', updated_at = NOW() WHERE map_id = $1",
        )
        .bind(map_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("mark ready: {e}")))?;
        tx.commit().await.map_err(|e| {
            DocumentMapRepositoryError::Internal(format!("commit project_insights: {e}"))
        })?;
        Ok(())
    }

    async fn project_failure(
        &self,
        map_id: Uuid,
        reason: &str,
    ) -> Result<(), DocumentMapRepositoryError> {
        sqlx::query(
            "UPDATE document_maps SET status = 'failed', failure_reason = $2, updated_at = NOW()
             WHERE map_id = $1",
        )
        .bind(map_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .map_err(|e| DocumentMapRepositoryError::Internal(format!("project_failure: {e}")))?;
        Ok(())
    }
}

fn internal<E: Display>(e: E) -> DocumentMapRepositoryError {
    DocumentMapRepositoryError::Internal(e.to_string())
}
