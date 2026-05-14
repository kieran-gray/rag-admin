use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use reqwest::Method;
use serde::Deserialize;

use crate::server::application::source_document::ports::source_adapter::{
    DocumentSummary, FetchedDocument, SourceAdapter,
};
use crate::server::application::AppError;
use crate::server::domain::source_document::{
    document_type::DocumentType,
    source_ref::SourceRef,
    version::{BlogPostMetadata, DocumentMetadata},
};
use crate::server::infrastructure::http_client::ReqwestHttpClient;

pub struct HttpBlogAdapter {
    http: Arc<ReqwestHttpClient>,
    blog_url: String,
}

impl HttpBlogAdapter {
    pub fn new(http: Arc<ReqwestHttpClient>, blog_url: String) -> Arc<Self> {
        Arc::new(Self { http, blog_url })
    }

    fn base_url(&self) -> Result<String, AppError> {
        let url = self.blog_url.trim().trim_end_matches('/').to_string();
        if url.is_empty() {
            return Err(AppError::Validation("blog URL is not configured".into()));
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, AppError> {
        let (status, body) = self
            .http
            .request_text(Method::GET, url, HeaderMap::new(), None)
            .await?;
        if !(200..300).contains(&status) {
            return Err(AppError::Upstream(format!(
                "GET {url} returned {status}: {}",
                truncate(&body, 300)
            )));
        }
        serde_json::from_str(&body).map_err(|e| AppError::Upstream(format!("parse {url}: {e}")))
    }
}

#[derive(Debug, Deserialize)]
struct PostsListResponse {
    posts: Vec<PostSummaryWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostSummaryWire {
    slug: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostDetailWire {
    title: String,
    published_at: String,
    markdown_body: String,
}

#[async_trait]
impl SourceAdapter for HttpBlogAdapter {
    fn document_type(&self) -> DocumentType {
        DocumentType::BlogPost
    }

    async fn list(&self) -> Result<Vec<DocumentSummary>, AppError> {
        let base = self.base_url()?;
        let url = format!("{base}/api/posts/index.json");
        let res: PostsListResponse = self.get_json(&url).await?;
        Ok(res
            .posts
            .into_iter()
            .map(|p| DocumentSummary {
                source_ref: SourceRef::UpstreamSlug { slug: p.slug },
                title: p.title,
            })
            .collect())
    }

    async fn fetch(&self, source_ref: &SourceRef) -> Result<FetchedDocument, AppError> {
        let SourceRef::UpstreamSlug { slug } = source_ref;
        let base = self.base_url()?;
        let url = format!("{base}/api/posts/{slug}.json");
        let detail: PostDetailWire = self.get_json(&url).await?;

        Ok(FetchedDocument {
            source_ref: source_ref.clone(),
            content: detail.markdown_body.into_bytes(),
            metadata: DocumentMetadata::BlogPost(BlogPostMetadata {
                title: detail.title,
                published_at: detail.published_at,
            }),
        })
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut s = s.chars().take(n).collect::<String>();
        s.push('\u{2026}');
        s
    }
}
