use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuestionCategory {
    #[default]
    FactRetrieval,
    Architecture,
    Reasoning,
    Code,
    Trick,
}

impl QuestionCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FactRetrieval => "fact_retrieval",
            Self::Architecture => "architecture",
            Self::Reasoning => "reasoning",
            Self::Code => "code",
            Self::Trick => "trick",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FactRetrieval => "Fact retrieval",
            Self::Architecture => "Architecture",
            Self::Reasoning => "Reasoning",
            Self::Code => "Code",
            Self::Trick => "Trick",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "fact_retrieval" => Ok(Self::FactRetrieval),
            "architecture" => Ok(Self::Architecture),
            "reasoning" => Ok(Self::Reasoning),
            "code" => Ok(Self::Code),
            "trick" => Ok(Self::Trick),
            other => Err(format!("unknown question category '{other}'")),
        }
    }

    pub fn all() -> [QuestionCategory; 5] {
        [
            Self::FactRetrieval,
            Self::Architecture,
            Self::Reasoning,
            Self::Code,
            Self::Trick,
        ]
    }
}
