use std::collections::BTreeMap;

use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::server::domain::source_document::{
    document_type::DocumentType,
    read_model::SourceDocumentReadModel,
    repository::{
        DocumentListCursor, DocumentListEntry, DocumentListPage, DocumentListQuery,
        DocumentStatusFilter, SourceDocumentRepository, SourceDocumentRepositoryError,
        SourceFilter,
    },
    source_ref::SourceRef,
    version::{ContentHash, DocumentMetadata},
};

pub struct PostgresSourceDocumentRepository {
    pool: PgPool,
}

impl PostgresSourceDocumentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SourceDocumentRepository for PostgresSourceDocumentRepository {
    async fn load(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
        let row: Option<SourceDocumentRow> = sqlx::query_as(
            "
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            WHERE document_id = $1
            ",
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("load: {e}")))?;

        row.map(SourceDocumentReadModel::try_from).transpose()
    }

    async fn save(
        &self,
        read_model: SourceDocumentReadModel,
    ) -> Result<(), SourceDocumentRepositoryError> {
        let document_type = serde_json::to_value(&read_model.document_type)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{:?}", read_model.document_type));
        let source_ref = serde_json::to_value(&read_model.source_ref).map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!("serialize source_ref: {e}"))
        })?;
        let latest_metadata = serde_json::to_value(&read_model.latest_metadata).map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!("serialize latest_metadata: {e}"))
        })?;

        sqlx::query(
            "
            INSERT INTO source_documents (
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at,
                deleted, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (document_id) DO UPDATE SET
                document_type = EXCLUDED.document_type,
                source_ref = EXCLUDED.source_ref,
                latest_version_number = EXCLUDED.latest_version_number,
                latest_content_hash = EXCLUDED.latest_content_hash,
                latest_metadata = EXCLUDED.latest_metadata,
                latest_version_occurred_at = EXCLUDED.latest_version_occurred_at,
                deleted = EXCLUDED.deleted,
                updated_at = NOW()
            ",
        )
        .bind(read_model.document_id)
        .bind(&document_type)
        .bind(&source_ref)
        .bind(read_model.latest_version_number.cast_signed())
        .bind(read_model.latest_content_hash.as_hex())
        .bind(&latest_metadata)
        .bind(&read_model.latest_version_occurred_at)
        .bind(read_model.deleted)
        .execute(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("save: {e}")))?;

        Ok(())
    }

    async fn list(&self) -> Result<Vec<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
        let rows: Vec<SourceDocumentRow> = sqlx::query_as(
            "
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            ORDER BY updated_at DESC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("list: {e}")))?;

        rows.into_iter()
            .map(SourceDocumentReadModel::try_from)
            .collect()
    }

    async fn list_page(
        &self,
        query: &DocumentListQuery,
    ) -> Result<DocumentListPage, SourceDocumentRepositoryError> {
        let limit = query.limit.clamp(1, 200) as i64;
        let fetch_limit = limit + 1;

        let total_matching: i64 = {
            let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
                "SELECT COUNT(*)::BIGINT FROM source_documents WHERE deleted = FALSE",
            );
            push_filters(&mut qb, query);
            qb.build_query_scalar::<i64>()
                .fetch_one(&self.pool)
                .await
                .map_err(|e| SourceDocumentRepositoryError::Internal(format!("count: {e}")))?
        };

        let total_all: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM source_documents WHERE deleted = FALSE",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("count_all: {e}")))?;

        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at_text
            FROM source_documents
            WHERE deleted = FALSE",
        );
        push_filters(&mut qb, query);
        if let Some(cursor) = &query.cursor {
            qb.push(" AND (updated_at, document_id) < (");
            qb.push_bind(parse_cursor_timestamp(&cursor.updated_at)?);
            qb.push(", ");
            qb.push_bind(cursor.document_id);
            qb.push(")");
        }
        qb.push(" ORDER BY updated_at DESC, document_id DESC LIMIT ");
        qb.push_bind(fetch_limit);

        let rows = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SourceDocumentRepositoryError::Internal(format!("list_page: {e}")))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let updated_at: String = row
                .try_get("updated_at_text")
                .map_err(|ref e| map_row_err(e))?;
            let raw = SourceDocumentRow {
                document_id: row.try_get("document_id").map_err(|ref e| map_row_err(e))?,
                document_type: row
                    .try_get("document_type")
                    .map_err(|ref e| map_row_err(e))?,
                source_ref: row.try_get("source_ref").map_err(|ref e| map_row_err(e))?,
                latest_version_number: row
                    .try_get("latest_version_number")
                    .map_err(|ref e| map_row_err(e))?,
                latest_content_hash: row
                    .try_get("latest_content_hash")
                    .map_err(|ref e| map_row_err(e))?,
                latest_metadata: row
                    .try_get("latest_metadata")
                    .map_err(|ref e| map_row_err(e))?,
                latest_version_occurred_at: row
                    .try_get("latest_version_occurred_at")
                    .map_err(|ref e| map_row_err(e))?,
                deleted: row.try_get("deleted").map_err(|ref e| map_row_err(e))?,
            };
            let model = SourceDocumentReadModel::try_from(raw)?;
            entries.push(DocumentListEntry {
                document: model,
                updated_at,
            });
        }

        let next_cursor = if entries.len() as i64 > limit {
            entries.pop();
            entries.last().map(|e| DocumentListCursor {
                updated_at: e.updated_at.clone(),
                document_id: e.document.document_id,
            })
        } else {
            None
        };

        Ok(DocumentListPage {
            items: entries,
            next_cursor,
            total_matching: total_matching.max(0) as u64,
            total_all: total_all.max(0) as u64,
        })
    }

    async fn source_facets(
        &self,
    ) -> Result<Vec<(SourceFilter, u64)>, SourceDocumentRepositoryError> {
        let rows = sqlx::query(
            "
            SELECT
                source_ref->>'type' AS kind,
                source_ref->'data'->>'url' AS url,
                COUNT(*)::BIGINT AS count
            FROM source_documents
            WHERE deleted = FALSE
            GROUP BY 1, 2
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("source_facets: {e}")))?;

        let mut upload_count: u64 = 0;
        let mut hosts: BTreeMap<String, u64> = BTreeMap::new();
        for row in rows {
            let kind: Option<String> = row.try_get("kind").map_err(|ref e| map_row_err(e))?;
            let url: Option<String> = row.try_get("url").map_err(|ref e| map_row_err(e))?;
            let count: i64 = row.try_get("count").map_err(|ref e| map_row_err(e))?;
            let count = count.max(0) as u64;
            match kind.as_deref() {
                Some("Upload") => upload_count += count,
                Some("Url") => {
                    let host = url
                        .as_deref()
                        .and_then(host_from_url)
                        .unwrap_or_else(|| "unknown".to_string());
                    *hosts.entry(host).or_default() += count;
                }
                _ => {}
            }
        }

        let mut out: Vec<(SourceFilter, u64)> = Vec::with_capacity(hosts.len() + 1);
        if upload_count > 0 {
            out.push((SourceFilter::Upload, upload_count));
        }
        for (host, count) in hosts {
            out.push((SourceFilter::UrlHost(host), count));
        }
        out.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| filter_sort_key(&a.0).cmp(&filter_sort_key(&b.0)))
        });
        Ok(out)
    }

    async fn find_by_source_ref(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
        let source_ref_json = serde_json::to_value(source_ref).map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!("serialize source_ref: {e}"))
        })?;

        let row: Option<SourceDocumentRow> = sqlx::query_as(
            "
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            WHERE source_ref = $1
            LIMIT 1
            ",
        )
        .bind(&source_ref_json)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("find_by_source_ref: {e}")))?;

        row.map(SourceDocumentReadModel::try_from).transpose()
    }
}

fn push_filters(qb: &mut QueryBuilder<'_, Postgres>, query: &DocumentListQuery) {
    if let Some(search) = query
        .search
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let pattern = format!(
            "%{}%",
            search
                .to_lowercase()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        qb.push(" AND (lower(coalesce(latest_metadata->>'title', '')) LIKE ");
        qb.push_bind(pattern.clone());
        qb.push(" ESCAPE '\\' OR lower(coalesce(source_ref->'data'->>'url', '')) LIKE ");
        qb.push_bind(pattern);
        qb.push(" ESCAPE '\\')");
    }

    if !query.sources.is_empty() {
        qb.push(" AND (");
        let mut first = true;
        for source in &query.sources {
            if !first {
                qb.push(" OR ");
            }
            first = false;
            match source {
                SourceFilter::Upload => {
                    qb.push("source_ref->>'type' = 'Upload'");
                }
                SourceFilter::UrlHost(host) => {
                    qb.push("(source_ref->>'type' = 'Url' AND lower(coalesce(source_ref->'data'->>'url', '')) ~ ");
                    let pattern =
                        format!("^https?://{}(:|/|$)", regex_escape(&host.to_lowercase()));
                    qb.push_bind(pattern);
                    qb.push(")");
                }
            }
        }
        qb.push(")");
    }

    if !query.statuses.is_empty() {
        qb.push(" AND (");
        let mut first = true;
        for status in &query.statuses {
            if !first {
                qb.push(" OR ");
            }
            first = false;
            match status {
                DocumentStatusFilter::Registered => {
                    qb.push("NOT EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE)");
                }
                DocumentStatusFilter::Indexed => {
                    qb.push("EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE AND i.status = 'indexed')");
                }
                DocumentStatusFilter::Failed => {
                    qb.push("EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE AND i.status = 'failed') AND NOT EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE AND i.status = 'indexed')");
                }
                DocumentStatusFilter::InProgress => {
                    qb.push("EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE AND i.status IN ('pending', 'chunking', 'embedding')) AND NOT EXISTS (SELECT 1 FROM indexings i WHERE i.document_id = source_documents.document_id AND i.removed = FALSE AND i.status IN ('indexed', 'failed'))");
                }
            }
        }
        qb.push(")");
    }
}

fn parse_cursor_timestamp(
    value: &str,
) -> Result<time::OffsetDateTime, SourceDocumentRepositoryError> {
    time::OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("cursor timestamp: {e}")))
}

fn map_row_err(e: &sqlx::Error) -> SourceDocumentRepositoryError {
    SourceDocumentRepositoryError::Internal(format!("row: {e}"))
}

fn regex_escape(s: &str) -> String {
    let specials = [
        '.', '+', '*', '?', '(', ')', '[', ']', '{', '}', '|', '^', '$', '\\',
    ];
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if specials.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

fn filter_sort_key(f: &SourceFilter) -> String {
    match f {
        SourceFilter::Upload => "\u{0000}upload".to_string(),
        SourceFilter::UrlHost(h) => format!("url:{h}"),
    }
}

#[derive(sqlx::FromRow)]
struct SourceDocumentRow {
    document_id: Uuid,
    document_type: String,
    source_ref: serde_json::Value,
    latest_version_number: i32,
    latest_content_hash: String,
    latest_metadata: serde_json::Value,
    latest_version_occurred_at: String,
    deleted: bool,
}

impl TryFrom<SourceDocumentRow> for SourceDocumentReadModel {
    type Error = SourceDocumentRepositoryError;

    fn try_from(row: SourceDocumentRow) -> Result<Self, Self::Error> {
        let document_type: DocumentType = serde_json::from_value(serde_json::Value::String(
            row.document_type.clone(),
        ))
        .map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!(
                "deserialize document_type '{}': {e}",
                row.document_type
            ))
        })?;
        let source_ref: SourceRef = serde_json::from_value(row.source_ref).map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!("deserialize source_ref: {e}"))
        })?;
        let latest_metadata: DocumentMetadata = serde_json::from_value(row.latest_metadata)
            .map_err(|e| {
                SourceDocumentRepositoryError::Internal(format!("deserialize latest_metadata: {e}"))
            })?;

        Ok(Self {
            document_id: row.document_id,
            document_type,
            source_ref,
            latest_version_number: row.latest_version_number.cast_unsigned(),
            latest_content_hash: ContentHash::new(row.latest_content_hash),
            latest_metadata,
            latest_version_occurred_at: row.latest_version_occurred_at,
            deleted: row.deleted,
        })
    }
}
