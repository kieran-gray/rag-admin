use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CognitiveOperation {
    Recall,
    Comprehend,
    Apply,
    Analyse,
    Synthesise,
    Evaluate,
    Adversarial,
}

impl CognitiveOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recall => "recall",
            Self::Comprehend => "comprehend",
            Self::Apply => "apply",
            Self::Analyse => "analyse",
            Self::Synthesise => "synthesise",
            Self::Evaluate => "evaluate",
            Self::Adversarial => "adversarial",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Recall => "Recall",
            Self::Comprehend => "Comprehend",
            Self::Apply => "Apply",
            Self::Analyse => "Analyse",
            Self::Synthesise => "Synthesise",
            Self::Evaluate => "Evaluate",
            Self::Adversarial => "Adversarial",
        }
    }

    pub fn definition(self) -> &'static str {
        match self {
            Self::Recall => "look up a single fact stated by the source",
            Self::Comprehend => "restate the source's meaning in your own words",
            Self::Apply => "use the source's content in a new but plausible context",
            Self::Analyse => "decompose the source into parts and compare them",
            Self::Synthesise => "combine multiple parts of the source into a new understanding",
            Self::Evaluate => "judge the source against a stated criterion",
            Self::Adversarial => "ask something the source does not actually answer",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "recall" => Ok(Self::Recall),
            "comprehend" => Ok(Self::Comprehend),
            "apply" => Ok(Self::Apply),
            "analyse" | "analyze" => Ok(Self::Analyse),
            "synthesise" | "synthesize" => Ok(Self::Synthesise),
            "evaluate" => Ok(Self::Evaluate),
            "adversarial" => Ok(Self::Adversarial),
            other => Err(format!("unknown cognitive operation '{other}'")),
        }
    }

    pub fn all() -> [Self; 7] {
        [
            Self::Recall,
            Self::Comprehend,
            Self::Apply,
            Self::Analyse,
            Self::Synthesise,
            Self::Evaluate,
            Self::Adversarial,
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Observation,
    Thread,
    Insight,
    Connection,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observation => "observation",
            Self::Thread => "thread",
            Self::Insight => "insight",
            Self::Connection => "connection",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Observation => "Observation",
            Self::Thread => "Thread",
            Self::Insight => "Insight",
            Self::Connection => "Connection",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "observation" => Ok(Self::Observation),
            "thread" => Ok(Self::Thread),
            "insight" => Ok(Self::Insight),
            "connection" => Ok(Self::Connection),
            other => Err(format!("unknown evidence kind '{other}'")),
        }
    }

    pub fn all() -> [Self; 4] {
        [
            Self::Observation,
            Self::Thread,
            Self::Insight,
            Self::Connection,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct QuestionDimensions {
    pub operation: CognitiveOperation,
    pub evidence: EvidenceKind,
    pub role: String,
}

pub fn is_combination_valid(op: CognitiveOperation, ev: EvidenceKind) -> bool {
    use CognitiveOperation::{
        Adversarial, Analyse, Apply, Comprehend, Evaluate, Recall, Synthesise,
    };
    use EvidenceKind::{Connection, Insight, Observation, Thread};
    matches!(
        (op, ev),
        (Recall, Observation)
            | (Comprehend, Observation | Thread)
            | (Apply | Analyse, Thread | Insight)
            | (Synthesise | Evaluate, Insight | Connection)
            | (Adversarial, _)
    )
}

pub fn allowed_evidence_for(op: CognitiveOperation) -> Vec<EvidenceKind> {
    EvidenceKind::all()
        .into_iter()
        .filter(|e| is_combination_valid(op, *e))
        .collect()
}
