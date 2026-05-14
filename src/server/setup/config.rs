use std::{any::type_name, env, str::FromStr};

use super::exceptions::SetupError;

trait FromEnv: Sized {
    fn from_env() -> Result<Self, SetupError>;
}

#[derive(Clone)]
pub struct Config {
    pub blog_url: String,
    pub database_url: String,
    pub cloudflare: CloudflareConfig,
    pub ollama: OllamaConfig,
    pub kv_backend: KvBackend,
}

#[derive(Clone)]
pub struct CloudflareConfig {
    pub account_id: String,
    pub api_token: String,
    pub kv_namespace_id: Option<String>,
}

#[derive(Clone)]
pub struct OllamaConfig {
    pub base_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KvBackend {
    Cloudflare,
    Postgres,
}

impl FromStr for KvBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cloudflare" => Ok(Self::Cloudflare),
            "postgres" => Ok(Self::Postgres),
            other => Err(format!("unknown KV backend '{other}'")),
        }
    }
}

impl FromEnv for CloudflareConfig {
    fn from_env() -> Result<Self, SetupError> {
        Ok(Self {
            account_id: Config::parse("CLOUDFLARE_ACCOUNT_ID")?,
            api_token: Config::parse("CLOUDFLARE_API_TOKEN")?,
            kv_namespace_id: Config::parse_optional("CLOUDFLARE_KV_NAMESPACE_ID"),
        })
    }
}

impl FromEnv for OllamaConfig {
    fn from_env() -> Result<Self, SetupError> {
        Ok(Self {
            base_url: Config::parse_optional("OLLAMA_BASE_URL")
                .unwrap_or_else(|| "http://localhost:11434".into()),
        })
    }
}

impl Config {
    pub fn from_env() -> Result<Self, SetupError> {
        let cloudflare = CloudflareConfig::from_env()?;
        let kv_backend = Self::parse_optional::<KvBackend>("KV_BACKEND").unwrap_or_else(|| {
            if cloudflare.kv_namespace_id.is_some() {
                KvBackend::Cloudflare
            } else {
                KvBackend::Postgres
            }
        });
        if kv_backend == KvBackend::Cloudflare && cloudflare.kv_namespace_id.is_none() {
            return Err(SetupError::MissingVariable(
                "CLOUDFLARE_KV_NAMESPACE_ID (required when KV_BACKEND=cloudflare)".to_owned(),
            ));
        }
        Ok(Self {
            blog_url: Self::parse("BLOG_URL")?,
            database_url: Self::parse("DATABASE_URL")?,
            cloudflare,
            ollama: OllamaConfig::from_env()?,
            kv_backend,
        })
    }

    fn parse<T: FromStr>(var: &str) -> Result<T, SetupError> {
        let type_name = type_name::<T>();
        env::var(var)
            .map_err(|_| SetupError::MissingVariable(var.to_owned()))?
            .parse()
            .map_err(|_| SetupError::InvalidVariable(format!("{var} must be {type_name}")))
    }

    fn parse_optional<T: FromStr>(var: &str) -> Option<T> {
        env::var(var).ok()?.parse().ok()
    }
}
