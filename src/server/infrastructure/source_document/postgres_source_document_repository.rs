use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::source_document::{
    document_type::DocumentType,
    read_model::SourceDocumentReadModel,
    repository::{SourceDocumentRepository, SourceDocumentRepositoryError},
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
            r#"
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            WHERE document_id = $1
            "#,
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
            r#"
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
            "#,
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
            r#"
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("list: {e}")))?;

        rows.into_iter()
            .map(SourceDocumentReadModel::try_from)
            .collect()
    }

    async fn find_by_source_ref(
        &self,
        source_ref: &SourceRef,
    ) -> Result<Option<SourceDocumentReadModel>, SourceDocumentRepositoryError> {
        let source_ref_json = serde_json::to_value(source_ref).map_err(|e| {
            SourceDocumentRepositoryError::Internal(format!("serialize source_ref: {e}"))
        })?;

        let row: Option<SourceDocumentRow> = sqlx::query_as(
            r#"
            SELECT
                document_id, document_type, source_ref, latest_version_number,
                latest_content_hash, latest_metadata, latest_version_occurred_at, deleted
            FROM source_documents
            WHERE source_ref = $1
            LIMIT 1
            "#,
        )
        .bind(&source_ref_json)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SourceDocumentRepositoryError::Internal(format!("find_by_source_ref: {e}")))?;

        row.map(SourceDocumentReadModel::try_from).transpose()
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
