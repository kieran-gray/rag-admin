use std::sync::Arc;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::map::read_model::DocumentMapReadModel;
use crate::server::domain::comprehension::map::repository::DocumentMapRepository;
use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::comprehension::span::Span;
use crate::server::domain::comprehension::thread::Thread;
use crate::shared::contracts::{
    DocumentMapDetailDto, DocumentMapSummaryDto, InsightDto, MapItemRefDto, ObservationDto,
    SpanDto, SuggestedRoleDto, ThreadDto,
};

pub struct ComprehensionQueryService {
    repository: Arc<dyn DocumentMapRepository>,
}

impl ComprehensionQueryService {
    pub fn new(repository: Arc<dyn DocumentMapRepository>) -> Arc<Self> {
        Arc::new(Self { repository })
    }

    pub async fn get_for_document(
        &self,
        document_id: Uuid,
        document_version: u32,
    ) -> Result<Option<DocumentMapDetailDto>, AppError> {
        let Some(summary) = self
            .repository
            .find_for_document(document_id, document_version)
            .await?
        else {
            return Ok(None);
        };
        self.get_detail(summary.map_id).await
    }

    pub async fn get_detail(&self, map_id: Uuid) -> Result<Option<DocumentMapDetailDto>, AppError> {
        let Some(detail) = self.repository.load_detail(map_id).await? else {
            return Ok(None);
        };
        Ok(Some(DocumentMapDetailDto {
            summary: summary_to_dto(&detail.read_model),
            observations: detail.observations.iter().map(observation_to_dto).collect(),
            threads: detail.threads.iter().map(thread_to_dto).collect(),
            insights: detail.insights.iter().map(insight_to_dto).collect(),
            carried_summary: detail.carried_summary,
        }))
    }

    pub async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentMapSummaryDto>, AppError> {
        let rows = self.repository.list_for_document(document_id).await?;
        Ok(rows.iter().map(summary_to_dto).collect())
    }
}

fn summary_to_dto(r: &DocumentMapReadModel) -> DocumentMapSummaryDto {
    DocumentMapSummaryDto {
        map_id: r.map_id,
        document_id: r.document_id,
        document_version: r.document_version,
        content_hash: r.content_hash.clone(),
        chunk_set_id: r.chunk_set_id,
        chunk_count: r.chunk_count,
        section_size: r.section_size,
        status: r.status.clone(),
        failure_reason: r.failure_reason.clone(),
        generation_model_id: r.generation_model_id,
        suggested_roles: r.suggested_roles.iter().map(role_to_dto).collect(),
        observations_extracted: r.observations_extracted,
        threads_synthesized: r.threads_synthesized,
        insights_synthesized: r.insights_synthesized,
    }
}

fn role_to_dto(r: &SuggestedRole) -> SuggestedRoleDto {
    SuggestedRoleDto {
        name: r.name.clone(),
        focus: r.focus.clone(),
    }
}

fn observation_to_dto(o: &Observation) -> ObservationDto {
    ObservationDto {
        observation_id: o.observation_id,
        chunk_sequence: o.chunk_sequence,
        kind: o.kind.clone(),
        summary: o.summary.clone(),
        spans: o.spans.iter().map(span_to_dto).collect(),
    }
}

fn thread_to_dto(t: &Thread) -> ThreadDto {
    ThreadDto {
        thread_id: t.thread_id,
        section_sequence: t.section_sequence,
        kind: t.kind.clone(),
        summary: t.summary.clone(),
        evidence: t.evidence.iter().map(map_item_ref_to_dto).collect(),
        spans: t.spans.iter().map(span_to_dto).collect(),
    }
}

fn insight_to_dto(i: &Insight) -> InsightDto {
    InsightDto {
        insight_id: i.insight_id,
        kind: i.kind.clone(),
        summary: i.summary.clone(),
        evidence: i.evidence.iter().map(map_item_ref_to_dto).collect(),
        spans: i.spans.iter().map(span_to_dto).collect(),
    }
}

pub fn span_to_dto(s: &Span) -> SpanDto {
    SpanDto {
        document_id: s.document_id,
        char_start: s.char_start,
        char_end: s.char_end,
    }
}

pub fn map_item_ref_to_dto(r: &MapItemRef) -> MapItemRefDto {
    match *r {
        MapItemRef::Observation {
            map_id,
            observation_id,
        } => MapItemRefDto::Observation {
            map_id,
            observation_id,
        },
        MapItemRef::Thread { map_id, thread_id } => MapItemRefDto::Thread { map_id, thread_id },
        MapItemRef::Insight { map_id, insight_id } => MapItemRefDto::Insight { map_id, insight_id },
        MapItemRef::Connection {
            corpus_map_id,
            connection_id,
        } => MapItemRefDto::Connection {
            corpus_map_id,
            connection_id,
        },
    }
}
