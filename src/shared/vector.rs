use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum VectorIndexConfig {
    Cloudflare {
        name: String,
        #[serde(deserialize_with = "crate::shared::serde_compat::u32_from_string")]
        dimensions: u32,
    },
}

impl Default for VectorIndexConfig {
    fn default() -> Self {
        VectorIndexConfig::Cloudflare {
            name: "blog-chunks".into(),
            dimensions: 1024,
        }
    }
}

impl VectorIndexConfig {
    pub fn name(&self) -> &str {
        match self {
            Self::Cloudflare { name, .. } => name,
        }
    }
    pub fn dimensions(&self) -> u32 {
        match self {
            Self::Cloudflare { dimensions, .. } => *dimensions,
        }
    }
    pub fn provider(&self) -> VectorProvider {
        match self {
            Self::Cloudflare { .. } => VectorProvider::Cloudflare,
        }
    }
    pub fn with_dimensions(self, dims: u32) -> Self {
        match self {
            Self::Cloudflare { name, .. } => Self::Cloudflare {
                name,
                dimensions: dims,
            },
        }
    }
    pub fn with_name(self, new_name: String) -> Self {
        match self {
            Self::Cloudflare { dimensions, .. } => Self::Cloudflare {
                name: new_name,
                dimensions,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VectorProvider {
    Cloudflare,
}

impl VectorProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            VectorProvider::Cloudflare => "cloudflare",
        }
    }
}
