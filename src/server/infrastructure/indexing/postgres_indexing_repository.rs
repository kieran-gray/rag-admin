use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::indexing::{
    read_model::IndexingReadModel,
    repository::{IndexingRepository, IndexingRepositoryError},
    status::{IndexingStatus, IngestStage},
};
use crate::shared::ChunkingConfig;

pub struct PostgresIndexingRepository {
    pool: PgPool,
}

impl PostgresIndexingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IndexingRepository for PostgresIndexingRepository {
    async fn load(
        &self,
        indexing_id: Uuid,
    ) -> Result<Option<IndexingReadModel>, IndexingRepositoryError> {
        let row: Option<IndexingRow> = sqlx::query_as(
            r#"
            SELECT
                indexing_id, document_id, pipeline_configuration_id, document_version,
                chunking_config, chunk_set_id, embedding_set_id, status, failure_stage,
                attempts, removed, auto_advance
            FROM indexings
            WHERE indexing_id = $1
            "#,
        )
        .bind(indexing_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| IndexingRepositoryError::Internal(format!("load: {e}")))?;

        row.map(IndexingReadModel::try_from).transpose()
    }

    async fn save(&self, read_model: IndexingReadModel) -> Result<(), IndexingRepositoryError> {
        let chunking_config = serde_json::to_value(read_model.chunking_config).map_err(|e| {
            IndexingRepositoryError::Internal(format!("serialize chunking_config: {e}"))
        })?;
        let (status, failure_stage) = encode_status(&read_model.status);

        sqlx::query(
            r#"
            INSERT INTO indexings (
                indexing_id, document_id, pipeline_configuration_id, document_version,
                chunking_config, chunk_set_id, embedding_set_id, status, failure_stage,
                attempts, removed, auto_advance, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
            ON CONFLICT (indexing_id) DO UPDATE SET
                document_id = EXCLUDED.document_id,
                pipeline_configuration_id = EXCLUDED.pipeline_configuration_id,
                document_version = EXCLUDED.document_version,
                chunking_config = EXCLUDED.chunking_config,
                chunk_set_id = EXCLUDED.chunk_set_id,
                embedding_set_id = EXCLUDED.embedding_set_id,
                status = EXCLUDED.status,
                failure_stage = EXCLUDED.failure_stage,
                attempts = EXCLUDED.attempts,
                removed = EXCLUDED.removed,
                auto_advance = EXCLUDED.auto_advance,
                updated_at = NOW()
            "#,
        )
        .bind(read_model.indexing_id)
        .bind(read_model.document_id)
        .bind(read_model.pipeline_configuration_id)
        .bind(read_model.document_version as i32)
        .bind(&chunking_config)
        .bind(read_model.chunk_set_id)
        .bind(read_model.embedding_set_id)
        .bind(status)
        .bind(failure_stage)
        .bind(read_model.attempts as i32)
        .bind(read_model.removed)
        .bind(read_model.auto_advance)
        .execute(&self.pool)
        .await
        .map_err(|e| IndexingRepositoryError::Internal(format!("save: {e}")))?;

        Ok(())
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<IndexingReadModel>, IndexingRepositoryError> {
        let rows: Vec<IndexingRow> = sqlx::query_as(
            r#"
            SELECT
                indexing_id, document_id, pipeline_configuration_id, document_version,
                chunking_config, chunk_set_id, embedding_set_id, status, failure_stage,
                attempts, removed, auto_advance
            FROM indexings
            WHERE document_id = $1
            ORDER BY updated_at DESC
            "#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| IndexingRepositoryError::Internal(format!("list_for_document: {e}")))?;

        rows.into_iter().map(IndexingReadModel::try_from).collect()
    }
}

#[derive(sqlx::FromRow)]
struct IndexingRow {
    indexing_id: Uuid,
    document_id: Uuid,
    pipeline_configuration_id: Uuid,
    document_version: i32,
    chunking_config: serde_json::Value,
    chunk_set_id: Option<Uuid>,
    embedding_set_id: Option<Uuid>,
    status: String,
    failure_stage: Option<String>,
    attempts: i32,
    removed: bool,
    auto_advance: bool,
}

impl TryFrom<IndexingRow> for IndexingReadModel {
    type Error = IndexingRepositoryError;

    fn try_from(row: IndexingRow) -> Result<Self, Self::Error> {
        let chunking_config: ChunkingConfig =
            serde_json::from_value(row.chunking_config).map_err(|e| {
                IndexingRepositoryError::Internal(format!("deserialize chunking_config: {e}"))
            })?;
        let status = decode_status(&row.status, row.failure_stage.as_deref())?;

        Ok(Self {
            indexing_id: row.indexing_id,
            document_id: row.document_id,
            pipeline_configuration_id: row.pipeline_configuration_id,
            document_version: row.document_version as u32,
            chunking_config,
            chunk_set_id: row.chunk_set_id,
            embedding_set_id: row.embedding_set_id,
            status,
            attempts: row.attempts as u32,
            removed: row.removed,
            auto_advance: row.auto_advance,
        })
    }
}

fn encode_status(status: &IndexingStatus) -> (&'static str, Option<String>) {
    match status {
        IndexingStatus::Pending => ("pending", None),
        IndexingStatus::Chunking => ("chunking", None),
        IndexingStatus::Embedding => ("embedding", None),
        IndexingStatus::Indexed => ("indexed", None),
        IndexingStatus::Failed { stage } => ("failed", Some(stage.to_string())),
    }
}

fn decode_status(
    status: &str,
    failure_stage: Option<&str>,
) -> Result<IndexingStatus, IndexingRepositoryError> {
    match status {
        "pending" => Ok(IndexingStatus::Pending),
        "chunking" => Ok(IndexingStatus::Chunking),
        "embedding" => Ok(IndexingStatus::Embedding),
        "indexed" => Ok(IndexingStatus::Indexed),
        "failed" => {
            let stage_str = failure_stage.ok_or_else(|| {
                IndexingRepositoryError::Internal(
                    "status 'failed' but failure_stage is NULL".to_string(),
                )
            })?;
            let stage = match stage_str {
                "fetching" => IngestStage::Fetching,
                "chunking" => IngestStage::Chunking,
                "embedding" => IngestStage::Embedding,
                "indexing" => IngestStage::Indexing,
                other => {
                    return Err(IndexingRepositoryError::Internal(format!(
                        "unknown failure_stage '{other}'"
                    )));
                }
            };
            Ok(IndexingStatus::Failed { stage })
        }
        other => Err(IndexingRepositoryError::Internal(format!(
            "unknown status '{other}'"
        ))),
    }
}
