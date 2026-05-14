use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
use crate::server::domain::chunk_set::repository::{ChunkSetRepository, ChunkSetRepositoryError};

pub struct PostgresChunkSetRepository {
    pool: PgPool,
}

impl PostgresChunkSetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChunkSetRepository for PostgresChunkSetRepository {
    async fn save(
        &self,
        chunk_set: ChunkSet,
        chunks: Vec<Chunk>,
    ) -> Result<(), ChunkSetRepositoryError> {
        let chunking_config = serde_json::to_value(chunk_set.chunking_config).map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("serialize chunking_config: {e}"))
        })?;

        let mut tx =
            self.pool.begin().await.map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("begin transaction: {e}"))
            })?;

        sqlx::query(
            r#"
            INSERT INTO chunk_sets (chunk_set_id, document_id, document_version, chunking_config, created_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (chunk_set_id) DO NOTHING
            "#,
        )
        .bind(chunk_set.chunk_set_id)
        .bind(chunk_set.document_id)
        .bind(chunk_set.document_version.cast_signed())
        .bind(&chunking_config)
        .bind(&chunk_set.created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("save chunk_set: {e}")))?;

        for chunk in &chunks {
            sqlx::query(
                r#"
                INSERT INTO chunks (chunk_id, chunk_set_id, sequence, heading, text, char_start, char_end)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (chunk_id) DO NOTHING
                "#,
            )
            .bind(chunk.chunk_id)
            .bind(chunk.chunk_set_id)
            .bind(chunk.sequence.cast_signed())
            .bind(&chunk.heading)
            .bind(&chunk.text)
            .bind(chunk.char_start.cast_signed())
            .bind(chunk.char_end.cast_signed())
            .execute(&mut *tx)
            .await
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("save chunk: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("commit: {e}")))?;

        Ok(())
    }

    async fn load(&self, chunk_set_id: Uuid) -> Result<Option<ChunkSet>, ChunkSetRepositoryError> {
        let row: Option<ChunkSetRow> = sqlx::query_as(
            "SELECT chunk_set_id, document_id, document_version, chunking_config, created_at FROM chunk_sets WHERE chunk_set_id = $1",
        )
        .bind(chunk_set_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("load chunk_set: {e}")))?;

        row.map(ChunkSet::try_from).transpose()
    }

    async fn load_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<Chunk>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkRow> = sqlx::query_as(
            "SELECT chunk_id, chunk_set_id, sequence, heading, text, char_start, char_end FROM chunks WHERE chunk_set_id = $1 ORDER BY sequence ASC",
        )
        .bind(chunk_set_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("load chunks: {e}")))?;

        Ok(rows.into_iter().map(Chunk::from).collect())
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSet>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkSetRow> = sqlx::query_as(
            "SELECT chunk_set_id, document_id, document_version, chunking_config, created_at FROM chunk_sets WHERE document_id = $1 ORDER BY created_at DESC",
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("list chunk_sets: {e}")))?;

        rows.into_iter().map(ChunkSet::try_from).collect()
    }
}

struct ChunkSetRow {
    chunk_set_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    chunking_config: serde_json::Value,
    created_at: String,
}

impl sqlx::FromRow<'_, PgRow> for ChunkSetRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row as _;
        Ok(Self {
            chunk_set_id: row.try_get("chunk_set_id")?,
            document_id: row.try_get("document_id")?,
            document_version: row.try_get("document_version")?,
            chunking_config: row.try_get("chunking_config")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl TryFrom<ChunkSetRow> for ChunkSet {
    type Error = ChunkSetRepositoryError;

    fn try_from(row: ChunkSetRow) -> Result<Self, Self::Error> {
        Ok(ChunkSet {
            chunk_set_id: row.chunk_set_id,
            document_id: row.document_id,
            document_version: row.document_version.cast_unsigned(),
            chunking_config: serde_json::from_value(row.chunking_config).map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("deserialize chunking_config: {e}"))
            })?,
            created_at: row.created_at,
        })
    }
}

struct ChunkRow {
    chunk_id: Uuid,
    chunk_set_id: Uuid,
    sequence: i32,
    heading: String,
    text: String,
    char_start: i32,
    char_end: i32,
}

impl sqlx::FromRow<'_, PgRow> for ChunkRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row as _;
        Ok(Self {
            chunk_id: row.try_get("chunk_id")?,
            chunk_set_id: row.try_get("chunk_set_id")?,
            sequence: row.try_get("sequence")?,
            heading: row.try_get("heading")?,
            text: row.try_get("text")?,
            char_start: row.try_get("char_start")?,
            char_end: row.try_get("char_end")?,
        })
    }
}

impl From<ChunkRow> for Chunk {
    fn from(row: ChunkRow) -> Self {
        Chunk {
            chunk_id: row.chunk_id,
            chunk_set_id: row.chunk_set_id,
            sequence: row.sequence.cast_unsigned(),
            heading: row.heading,
            text: row.text,
            char_start: row.char_start.cast_unsigned(),
            char_end: row.char_end.cast_unsigned(),
        }
    }
}
