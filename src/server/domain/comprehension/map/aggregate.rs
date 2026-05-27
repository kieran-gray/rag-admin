use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::Aggregate;

use super::super::insight::Insight;
use super::super::observation::Observation;
use super::super::role::SuggestedRole;
use super::super::thread::Thread;
use super::commands::DocumentMapCommand;
use super::events::{
    ChunkExtractionFailed, DocumentMapEvent, InsightsSynthesized, MapBuildRequested, MapFailed,
    ObservationsExtracted, RolesSuggested, SectionSynthesisFailed, ThreadsSynthesized,
};
use super::exceptions::DocumentMapError;

const DOCUMENT_MAP_NAMESPACE: Uuid = uuid::uuid!("a8f10c43-29e8-4d6b-99c2-1bd31f8d2c47");

pub fn derive_map_id(document_id: Uuid, document_version: u32, content_hash: &str) -> Uuid {
    let name = format!("{document_id}|{document_version}|{content_hash}");
    Uuid::new_v5(&DOCUMENT_MAP_NAMESPACE, name.as_bytes())
}

pub fn section_count_for(chunk_count: u32, section_size: u32) -> u32 {
    if section_size == 0 || chunk_count == 0 {
        return 0;
    }
    chunk_count.div_ceil(section_size)
}

pub fn section_for_chunk(chunk_sequence: u32, section_size: u32) -> u32 {
    if section_size == 0 {
        return 0;
    }
    chunk_sequence / section_size
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MapStatus {
    Pending,
    TypingRoles,
    ExtractingObservations { remaining_chunks: u32 },
    SynthesizingThreads { remaining_sections: u32 },
    SynthesizingInsights,
    Ready,
    Failed { reason: String },
}

impl MapStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::TypingRoles => "typing_roles",
            Self::ExtractingObservations { .. } => "extracting_observations",
            Self::SynthesizingThreads { .. } => "synthesizing_threads",
            Self::SynthesizingInsights => "synthesizing_insights",
            Self::Ready => "ready",
            Self::Failed { .. } => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ready | Self::Failed { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMap {
    pub map_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub content_hash: String,
    pub chunk_set_id: Uuid,
    pub chunk_count: u32,
    pub section_size: u32,
    pub generation_model_id: Uuid,
    pub status: MapStatus,
    pub suggested_roles: Vec<SuggestedRole>,
    pub observations: Vec<Observation>,
    pub threads: Vec<Thread>,
    pub insights: Vec<Insight>,
    pub failure_reason: Option<String>,
    pub completed_chunks: Vec<u32>,
    pub completed_sections: Vec<u32>,
}

impl DocumentMap {
    fn from_requested(e: &MapBuildRequested) -> Self {
        Self {
            map_id: e.map_id,
            document_id: e.document_id,
            document_version: e.document_version,
            content_hash: e.content_hash.clone(),
            chunk_set_id: e.chunk_set_id,
            chunk_count: e.chunk_count,
            section_size: e.section_size,
            generation_model_id: e.generation_model_id,
            status: MapStatus::Pending,
            suggested_roles: Vec::new(),
            observations: Vec::new(),
            threads: Vec::new(),
            insights: Vec::new(),
            failure_reason: None,
            completed_chunks: Vec::new(),
            completed_sections: Vec::new(),
        }
    }

    pub fn section_count(&self) -> u32 {
        section_count_for(self.chunk_count, self.section_size)
    }

    fn record_chunk_completion(&mut self, chunk_sequence: u32) {
        if !self.completed_chunks.contains(&chunk_sequence) {
            self.completed_chunks.push(chunk_sequence);
        }
    }

    fn record_section_completion(&mut self, section_sequence: u32) {
        if !self.completed_sections.contains(&section_sequence) {
            self.completed_sections.push(section_sequence);
        }
    }

    fn last_carried_summary(&self) -> String {
        self.threads
            .iter()
            .max_by_key(|t| t.section_sequence)
            .map(|_t| String::new())
            .unwrap_or_default()
    }
}

impl Aggregate for DocumentMap {
    type Event = DocumentMapEvent;
    type Command = DocumentMapCommand;
    type Error = DocumentMapError;

    fn aggregate_type() -> &'static str {
        "document_map"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            DocumentMapEvent::MapBuildRequested(_) => {}
            DocumentMapEvent::RolesSuggested(e) => {
                self.suggested_roles.clone_from(&e.roles);
                if self.chunk_count == 0 {
                    self.status = MapStatus::SynthesizingInsights;
                } else {
                    self.status = MapStatus::ExtractingObservations {
                        remaining_chunks: self.chunk_count,
                    };
                }
            }
            DocumentMapEvent::ObservationsExtracted(e) => {
                self.observations.extend(e.observations.iter().cloned());
                self.record_chunk_completion(e.chunk_sequence);
                let remaining = self
                    .chunk_count
                    .saturating_sub(self.completed_chunks.len() as u32);
                if remaining == 0 {
                    let sections = self.section_count();
                    if sections == 0 {
                        self.status = MapStatus::SynthesizingInsights;
                    } else {
                        self.status = MapStatus::SynthesizingThreads {
                            remaining_sections: sections,
                        };
                    }
                } else {
                    self.status = MapStatus::ExtractingObservations {
                        remaining_chunks: remaining,
                    };
                }
            }
            DocumentMapEvent::ChunkExtractionFailed(e) => {
                self.record_chunk_completion(e.chunk_sequence);
                let remaining = self
                    .chunk_count
                    .saturating_sub(self.completed_chunks.len() as u32);
                if remaining == 0 {
                    let sections = self.section_count();
                    if sections == 0 {
                        self.status = MapStatus::SynthesizingInsights;
                    } else {
                        self.status = MapStatus::SynthesizingThreads {
                            remaining_sections: sections,
                        };
                    }
                } else {
                    self.status = MapStatus::ExtractingObservations {
                        remaining_chunks: remaining,
                    };
                }
            }
            DocumentMapEvent::ThreadsSynthesized(e) => {
                self.threads.extend(e.threads.iter().cloned());
                self.record_section_completion(e.section_sequence);
                let remaining = self
                    .section_count()
                    .saturating_sub(self.completed_sections.len() as u32);
                if remaining == 0 {
                    self.status = MapStatus::SynthesizingInsights;
                } else {
                    self.status = MapStatus::SynthesizingThreads {
                        remaining_sections: remaining,
                    };
                }
            }
            DocumentMapEvent::SectionSynthesisFailed(e) => {
                self.record_section_completion(e.section_sequence);
                let remaining = self
                    .section_count()
                    .saturating_sub(self.completed_sections.len() as u32);
                if remaining == 0 {
                    self.status = MapStatus::SynthesizingInsights;
                } else {
                    self.status = MapStatus::SynthesizingThreads {
                        remaining_sections: remaining,
                    };
                }
            }
            DocumentMapEvent::InsightsSynthesized(e) => {
                self.insights.extend(e.insights.iter().cloned());
                self.status = MapStatus::Ready;
            }
            DocumentMapEvent::MapFailed(e) => {
                self.failure_reason = Some(e.reason.clone());
                self.status = MapStatus::Failed {
                    reason: e.reason.clone(),
                };
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            DocumentMapCommand::RequestMapBuild(cmd) => {
                if state.is_some() {
                    return Err(DocumentMapError::AlreadyExists);
                }
                Ok(vec![DocumentMapEvent::MapBuildRequested(
                    MapBuildRequested {
                        map_id: cmd.map_id,
                        document_id: cmd.document_id,
                        document_version: cmd.document_version,
                        content_hash: cmd.content_hash,
                        chunk_set_id: cmd.chunk_set_id,
                        chunk_count: cmd.chunk_count,
                        section_size: cmd.section_size,
                        generation_model_id: cmd.generation_model_id,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::RecordSuggestedRoles(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if !matches!(map.status, MapStatus::Pending | MapStatus::TypingRoles) {
                    return Ok(vec![]);
                }
                Ok(vec![DocumentMapEvent::RolesSuggested(RolesSuggested {
                    roles: cmd.roles,
                    occurred_at: cmd.occurred_at,
                })])
            }
            DocumentMapCommand::RecordObservations(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if map.completed_chunks.contains(&cmd.chunk_sequence) {
                    return Ok(vec![]);
                }
                Ok(vec![DocumentMapEvent::ObservationsExtracted(
                    ObservationsExtracted {
                        chunk_sequence: cmd.chunk_sequence,
                        observations: cmd.observations,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::RecordChunkExtractionFailure(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if map.completed_chunks.contains(&cmd.chunk_sequence) {
                    return Ok(vec![]);
                }
                Ok(vec![DocumentMapEvent::ChunkExtractionFailed(
                    ChunkExtractionFailed {
                        chunk_sequence: cmd.chunk_sequence,
                        reason: cmd.reason,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::RecordThreads(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if map.completed_sections.contains(&cmd.section_sequence) {
                    return Ok(vec![]);
                }
                Ok(vec![DocumentMapEvent::ThreadsSynthesized(
                    ThreadsSynthesized {
                        section_sequence: cmd.section_sequence,
                        threads: cmd.threads,
                        carried_summary: cmd.carried_summary,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::RecordSectionSynthesisFailure(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if map.completed_sections.contains(&cmd.section_sequence) {
                    return Ok(vec![]);
                }
                Ok(vec![DocumentMapEvent::SectionSynthesisFailed(
                    SectionSynthesisFailed {
                        section_sequence: cmd.section_sequence,
                        reason: cmd.reason,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::RecordInsights(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                if map.status.is_terminal() {
                    return Ok(vec![]);
                }
                if !matches!(map.status, MapStatus::SynthesizingInsights) {
                    return Err(DocumentMapError::InvalidTransition(format!(
                        "cannot record insights while status is {}",
                        map.status.as_str()
                    )));
                }
                Ok(vec![DocumentMapEvent::InsightsSynthesized(
                    InsightsSynthesized {
                        insights: cmd.insights,
                        occurred_at: cmd.occurred_at,
                    },
                )])
            }
            DocumentMapCommand::FailMap(cmd) => {
                let map = state.ok_or(DocumentMapError::NotFound)?;
                match &map.status {
                    MapStatus::Ready => Err(DocumentMapError::AlreadyReady),
                    MapStatus::Failed { .. } => Ok(vec![]),
                    _ => Ok(vec![DocumentMapEvent::MapFailed(MapFailed {
                        reason: cmd.reason,
                        occurred_at: cmd.occurred_at,
                    })]),
                }
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;
        for event in events {
            match (&mut state, event) {
                (None, DocumentMapEvent::MapBuildRequested(e)) => {
                    state = Some(Self::from_requested(e));
                }
                (Some(_), DocumentMapEvent::MapBuildRequested(_)) | (None, _) => return None,
                (Some(map), event) => map.apply(event),
            }
        }
        state
    }
}

impl DocumentMap {
    pub fn carried_summary(&self) -> String {
        self.last_carried_summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::domain::comprehension::map::commands::{
        FailMap, RecordInsights, RecordObservations, RecordSuggestedRoles, RequestMapBuild,
    };
    use crate::server::domain::comprehension::role::SuggestedRole;
    use crate::server::domain::shared::Timestamp;

    fn ts() -> Timestamp {
        "2026-01-01T00:00:00Z".into()
    }

    fn request_cmd(chunk_count: u32) -> DocumentMapCommand {
        DocumentMapCommand::RequestMapBuild(RequestMapBuild {
            map_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            document_version: 1,
            content_hash: "h".into(),
            chunk_set_id: Uuid::new_v4(),
            chunk_count,
            section_size: 16,
            generation_model_id: Uuid::new_v4(),
            occurred_at: ts(),
        })
    }

    #[test]
    fn request_creates_pending_map() {
        let events = DocumentMap::handle_command(None, request_cmd(40)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        assert_eq!(state.status, MapStatus::Pending);
        assert_eq!(state.chunk_count, 40);
    }

    #[test]
    fn duplicate_request_fails() {
        let events = DocumentMap::handle_command(None, request_cmd(10)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        let err = DocumentMap::handle_command(Some(&state), request_cmd(10)).unwrap_err();
        assert!(matches!(err, DocumentMapError::AlreadyExists));
    }

    #[test]
    fn record_roles_transitions_to_extracting() {
        let mut events = DocumentMap::handle_command(None, request_cmd(32)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        let more = DocumentMap::handle_command(
            Some(&state),
            DocumentMapCommand::RecordSuggestedRoles(RecordSuggestedRoles {
                map_id: state.map_id,
                roles: vec![SuggestedRole {
                    name: "engineer".into(),
                    focus: "tradeoffs".into(),
                }],
                occurred_at: ts(),
            }),
        )
        .unwrap();
        events.extend(more);
        let state = DocumentMap::from_events(&events).unwrap();
        assert!(matches!(
            state.status,
            MapStatus::ExtractingObservations {
                remaining_chunks: 32
            }
        ));
        assert_eq!(state.suggested_roles.len(), 1);
    }

    #[test]
    fn observations_decrement_remaining_chunks() {
        let mut events = DocumentMap::handle_command(None, request_cmd(2)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordSuggestedRoles(RecordSuggestedRoles {
                    map_id: state.map_id,
                    roles: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordObservations(RecordObservations {
                    map_id: state.map_id,
                    chunk_sequence: 0,
                    observations: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        assert!(matches!(
            state.status,
            MapStatus::ExtractingObservations {
                remaining_chunks: 1
            }
        ));
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordObservations(RecordObservations {
                    map_id: state.map_id,
                    chunk_sequence: 1,
                    observations: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        assert!(matches!(
            state.status,
            MapStatus::SynthesizingThreads {
                remaining_sections: 1
            }
        ));
    }

    #[test]
    fn duplicate_chunk_observations_are_noop() {
        let mut events = DocumentMap::handle_command(None, request_cmd(1)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordSuggestedRoles(RecordSuggestedRoles {
                    map_id: state.map_id,
                    roles: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordObservations(RecordObservations {
                    map_id: state.map_id,
                    chunk_sequence: 0,
                    observations: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        let again = DocumentMap::handle_command(
            Some(&state),
            DocumentMapCommand::RecordObservations(RecordObservations {
                map_id: state.map_id,
                chunk_sequence: 0,
                observations: vec![],
                occurred_at: ts(),
            }),
        )
        .unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn record_insights_only_during_synth_insights() {
        let events = DocumentMap::handle_command(None, request_cmd(8)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        let err = DocumentMap::handle_command(
            Some(&state),
            DocumentMapCommand::RecordInsights(RecordInsights {
                map_id: state.map_id,
                insights: vec![],
                occurred_at: ts(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, DocumentMapError::InvalidTransition(_)));
    }

    #[test]
    fn fail_map_after_ready_errors() {
        let mut events = DocumentMap::handle_command(None, request_cmd(0)).unwrap();
        let state = DocumentMap::from_events(&events).unwrap();
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordSuggestedRoles(RecordSuggestedRoles {
                    map_id: state.map_id,
                    roles: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        assert!(matches!(state.status, MapStatus::SynthesizingInsights));
        events.extend(
            DocumentMap::handle_command(
                Some(&state),
                DocumentMapCommand::RecordInsights(RecordInsights {
                    map_id: state.map_id,
                    insights: vec![],
                    occurred_at: ts(),
                }),
            )
            .unwrap(),
        );
        let state = DocumentMap::from_events(&events).unwrap();
        assert!(matches!(state.status, MapStatus::Ready));
        let err = DocumentMap::handle_command(
            Some(&state),
            DocumentMapCommand::FailMap(FailMap {
                map_id: state.map_id,
                reason: "x".into(),
                occurred_at: ts(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, DocumentMapError::AlreadyReady));
    }

    #[test]
    fn section_helpers() {
        assert_eq!(section_count_for(0, 16), 0);
        assert_eq!(section_count_for(1, 16), 1);
        assert_eq!(section_count_for(16, 16), 1);
        assert_eq!(section_count_for(17, 16), 2);
        assert_eq!(section_for_chunk(0, 16), 0);
        assert_eq!(section_for_chunk(15, 16), 0);
        assert_eq!(section_for_chunk(16, 16), 1);
    }
}
