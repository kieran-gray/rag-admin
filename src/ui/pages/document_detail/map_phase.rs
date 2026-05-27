use crate::ui::components::primitives::Status;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MapPhase {
    Pending,
    TypingRoles,
    ExtractingObservations,
    SynthesizingThreads,
    SynthesizingInsights,
    Ready,
    Failed,
}

impl MapPhase {
    pub fn from_status(status: &str) -> Self {
        match status {
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            "typing_roles" => Self::TypingRoles,
            "extracting_observations" => Self::ExtractingObservations,
            "synthesizing_threads" => Self::SynthesizingThreads,
            "synthesizing_insights" => Self::SynthesizingInsights,
            _ => Self::Pending,
        }
    }

    pub fn order(self) -> u8 {
        match self {
            Self::Pending => 0,
            Self::TypingRoles => 1,
            Self::ExtractingObservations => 2,
            Self::SynthesizingThreads => 3,
            Self::SynthesizingInsights => 4,
            Self::Ready => 5,
            Self::Failed => 99,
        }
    }

    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_failed(self) -> bool {
        matches!(self, Self::Failed)
    }

    pub fn pill(self, progress: MapProgress) -> (Status, String) {
        match self {
            Self::Ready => (Status::Ok, "Ready".to_string()),
            Self::Failed => (Status::Fail, "Failed".to_string()),
            Self::Pending => (Status::Pending, "Queued".to_string()),
            Self::TypingRoles => (Status::Pending, "Typing roles".to_string()),
            Self::ExtractingObservations => (
                Status::Pending,
                format!(
                    "Observations {}/{}",
                    progress.observations_extracted, progress.chunk_count,
                ),
            ),
            Self::SynthesizingThreads => (
                Status::Pending,
                format!(
                    "Threads {}/{}",
                    progress.threads_synthesized,
                    section_count(progress.chunk_count, progress.section_size),
                ),
            ),
            Self::SynthesizingInsights => (Status::Pending, "Synthesizing insights".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MapProgress {
    pub observations_extracted: u32,
    pub chunk_count: u32,
    pub threads_synthesized: u32,
    pub section_size: u32,
}

pub fn section_count(chunk_count: u32, section_size: u32) -> u32 {
    if section_size == 0 || chunk_count == 0 {
        return 0;
    }
    chunk_count.div_ceil(section_size)
}
