use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrammarVariant {
    #[default]
    Clean,
    Broken,
}

impl GrammarVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Broken => "broken",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "Clean",
            Self::Broken => "Broken",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "clean" => Ok(Self::Clean),
            "broken" => Ok(Self::Broken),
            other => Err(format!("unknown grammar variant '{other}'")),
        }
    }
}
