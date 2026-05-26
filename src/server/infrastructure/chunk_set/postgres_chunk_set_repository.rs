use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::server::domain::chunk_set::chunk::Chunk;
use crate::server::domain::chunk_set::events::ChunkSetCreated;
use crate::server::domain::chunk_set::read_model::ChunkSetReadModel;
use crate::server::domain::chunk_set::repository::{
    ChunkSetListCursor, ChunkSetListPage, ChunkSetListQuery, ChunkSetRepository,
    ChunkSetRepositoryError, ChunkSetStatusFilter,
};

pub struct PostgresChunkSetRepository {
    pool: PgPool,
}

impl PostgresChunkSetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const SELECT_COLS: &str = "chunk_set_id, document_id, document_version, chunking_config, \
                           chunk_count, pinned, indexing_refs, variant_result_refs, created_at";

#[async_trait]
impl ChunkSetRepository for PostgresChunkSetRepository {
    async fn load_summary(
        &self,
        chunk_set_id: Uuid,
    ) -> Result<Option<ChunkSetReadModel>, ChunkSetRepositoryError> {
        let row: Option<ChunkSetRow> = sqlx::query_as(&format!(
            "SELECT {SELECT_COLS} FROM chunk_sets WHERE chunk_set_id = $1"
        ))
        .bind(chunk_set_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("load_summary: {e}")))?;

        row.map(ChunkSetReadModel::try_from).transpose()
    }

    async fn load_chunks(&self, chunk_set_id: Uuid) -> Result<Vec<Chunk>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkRow> = sqlx::query_as(
            "SELECT chunk_id, sequence, heading, text, char_start, char_end
             FROM chunks WHERE chunk_set_id = $1 ORDER BY sequence ASC",
        )
        .bind(chunk_set_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("load_chunks: {e}")))?;

        Ok(rows.into_iter().map(Chunk::from).collect())
    }

    async fn list_for_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkSetRow> = sqlx::query_as(&format!(
            "SELECT {SELECT_COLS} FROM chunk_sets
             WHERE document_id = $1
             ORDER BY created_at DESC, chunk_set_id DESC"
        ))
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("list_for_document: {e}")))?;

        rows.into_iter().map(ChunkSetReadModel::try_from).collect()
    }

    async fn list_page(
        &self,
        query: &ChunkSetListQuery,
    ) -> Result<ChunkSetListPage, ChunkSetRepositoryError> {
        let limit = query.limit.clamp(1, 200) as i64;
        let fetch_limit = limit + 1;

        let total_all: i64 =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*)::BIGINT FROM chunk_sets")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| ChunkSetRepositoryError::Internal(format!("count_all: {e}")))?;

        let total_matching: i64 = {
            let mut qb: QueryBuilder<Postgres> =
                QueryBuilder::new("SELECT COUNT(*)::BIGINT FROM chunk_sets WHERE 1=1");
            push_status_filter(&mut qb, &query.statuses);
            qb.build_query_scalar::<i64>()
                .fetch_one(&self.pool)
                .await
                .map_err(|e| ChunkSetRepositoryError::Internal(format!("count_matching: {e}")))?
        };

        let facet_row = sqlx::query(
            "SELECT
                COUNT(*) FILTER (WHERE pinned)                    AS pinned,
                COUNT(*) FILTER (WHERE indexing_refs > 0)         AS indexed,
                COUNT(*) FILTER (WHERE variant_result_refs > 0)   AS used_by_eval,
                COUNT(*) FILTER (WHERE NOT pinned
                                  AND indexing_refs = 0
                                  AND variant_result_refs = 0)    AS unused
             FROM chunk_sets",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("status_counts: {e}")))?;

        let status_counts: Vec<(ChunkSetStatusFilter, u64)> = vec![
            (
                ChunkSetStatusFilter::Pinned,
                read_facet(&facet_row, "pinned")?,
            ),
            (
                ChunkSetStatusFilter::Indexed,
                read_facet(&facet_row, "indexed")?,
            ),
            (
                ChunkSetStatusFilter::UsedByEval,
                read_facet(&facet_row, "used_by_eval")?,
            ),
            (
                ChunkSetStatusFilter::Unused,
                read_facet(&facet_row, "unused")?,
            ),
        ];

        let mut qb: QueryBuilder<Postgres> =
            QueryBuilder::new(&format!("SELECT {SELECT_COLS} FROM chunk_sets WHERE 1=1"));
        push_status_filter(&mut qb, &query.statuses);

        if let Some(cursor) = &query.cursor {
            qb.push(" AND (created_at, chunk_set_id) < (");
            qb.push_bind(cursor.created_at);
            qb.push(", ");
            qb.push_bind(cursor.chunk_set_id);
            qb.push(")");
        }
        qb.push(" ORDER BY created_at DESC, chunk_set_id DESC LIMIT ");
        qb.push_bind(fetch_limit);

        let rows: Vec<ChunkSetRow> = qb
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("list_page: {e}")))?;

        let mut items: Vec<ChunkSetReadModel> = rows
            .into_iter()
            .map(ChunkSetReadModel::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let next_cursor = if items.len() as i64 > limit {
            items.pop();
            items
                .last()
                .map(|m| -> Result<ChunkSetListCursor, ChunkSetRepositoryError> {
                    let parsed = OffsetDateTime::parse(&m.created_at, &Rfc3339).map_err(|e| {
                        ChunkSetRepositoryError::Internal(format!("cursor timestamp: {e}"))
                    })?;
                    Ok(ChunkSetListCursor {
                        created_at: parsed,
                        chunk_set_id: m.chunk_set_id,
                    })
                })
                .transpose()?
        } else {
            None
        };

        Ok(ChunkSetListPage {
            items,
            next_cursor,
            total_matching: total_matching.max(0) as u64,
            total_all: total_all.max(0) as u64,
            status_counts,
        })
    }

    async fn project_created(
        &self,
        event: &ChunkSetCreated,
    ) -> Result<(), ChunkSetRepositoryError> {
        let chunking_config = serde_json::to_value(event.chunking_config).map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("serialize chunking_config: {e}"))
        })?;
        let occurred_at = OffsetDateTime::parse(&event.occurred_at.to_string(), &Rfc3339)
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("parse occurred_at: {e}")))?;

        let mut tx =
            self.pool.begin().await.map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("begin transaction: {e}"))
            })?;

        sqlx::query(
            "INSERT INTO chunk_sets (
                chunk_set_id, document_id, document_version, chunking_config,
                chunk_count, pinned, indexing_refs, variant_result_refs, created_at
             )
             VALUES ($1, $2, $3, $4, $5, FALSE, 0, 0, $6)
             ON CONFLICT (chunk_set_id) DO NOTHING",
        )
        .bind(event.chunk_set_id)
        .bind(event.document_id)
        .bind(event.document_version as i32)
        .bind(&chunking_config)
        .bind(event.chunks.len() as i32)
        .bind(occurred_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("project_created chunk_sets: {e}"))
        })?;

        if !event.chunks.is_empty() {
            let chunk_ids: Vec<Uuid> = event.chunks.iter().map(|c| c.chunk_id).collect();
            let chunk_set_ids: Vec<Uuid> = vec![event.chunk_set_id; event.chunks.len()];
            let sequences: Vec<i32> = event.chunks.iter().map(|c| c.sequence as i32).collect();
            let headings: Vec<&str> = event.chunks.iter().map(|c| c.heading.as_str()).collect();
            let texts: Vec<&str> = event.chunks.iter().map(|c| c.text.as_str()).collect();
            let starts: Vec<i32> = event.chunks.iter().map(|c| c.char_start as i32).collect();
            let ends: Vec<i32> = event.chunks.iter().map(|c| c.char_end as i32).collect();

            sqlx::query(
                "INSERT INTO chunks (chunk_id, chunk_set_id, sequence, heading, text, char_start, char_end)
                 SELECT * FROM UNNEST(
                     $1::uuid[], $2::uuid[], $3::int[], $4::text[], $5::text[], $6::int[], $7::int[]
                 )
                 ON CONFLICT (chunk_id) DO NOTHING",
            )
            .bind(&chunk_ids)
            .bind(&chunk_set_ids)
            .bind(&sequences)
            .bind(&headings)
            .bind(&texts)
            .bind(&starts)
            .bind(&ends)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("project_created chunks: {e}"))
            })?;
        }

        tx.commit().await.map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("commit project_created: {e}"))
        })?;
        Ok(())
    }

    async fn project_pinned(
        &self,
        chunk_set_id: Uuid,
        pinned: bool,
    ) -> Result<(), ChunkSetRepositoryError> {
        sqlx::query("UPDATE chunk_sets SET pinned = $2 WHERE chunk_set_id = $1")
            .bind(chunk_set_id)
            .bind(pinned)
            .execute(&self.pool)
            .await
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("project_pinned: {e}")))?;
        Ok(())
    }

    async fn project_deleted(&self, chunk_set_id: Uuid) -> Result<(), ChunkSetRepositoryError> {
        match sqlx::query("DELETE FROM chunk_sets WHERE chunk_set_id = $1")
            .bind(chunk_set_id)
            .execute(&self.pool)
            .await
        {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db)) if db.is_foreign_key_violation() => {
                Err(ChunkSetRepositoryError::InUse(chunk_set_id))
            }
            Err(e) => Err(ChunkSetRepositoryError::Internal(format!(
                "project_deleted: {e}"
            ))),
        }
    }

    async fn bump_indexing_refs(
        &self,
        chunk_set_id: Uuid,
        delta: i32,
    ) -> Result<(), ChunkSetRepositoryError> {
        sqlx::query(
            "UPDATE chunk_sets
             SET indexing_refs = GREATEST(indexing_refs + $2, 0)
             WHERE chunk_set_id = $1",
        )
        .bind(chunk_set_id)
        .bind(delta)
        .execute(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("bump_indexing_refs: {e}")))?;
        Ok(())
    }

    async fn bump_variant_result_refs(
        &self,
        chunk_set_id: Uuid,
        delta: i32,
    ) -> Result<(), ChunkSetRepositoryError> {
        sqlx::query(
            "UPDATE chunk_sets
             SET variant_result_refs = GREATEST(variant_result_refs + $2, 0)
             WHERE chunk_set_id = $1",
        )
        .bind(chunk_set_id)
        .bind(delta)
        .execute(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("bump_variant_result_refs: {e}")))?;
        Ok(())
    }

    async fn reconcile_referrer_counts(&self) -> Result<(), ChunkSetRepositoryError> {
        sqlx::query(
            "WITH live_indexing AS (
                SELECT chunk_set_id, COUNT(*)::INT AS cnt
                FROM indexings
                WHERE chunk_set_id IS NOT NULL AND NOT removed
                GROUP BY chunk_set_id
            ), live_variants AS (
                SELECT chunk_set_id, COUNT(*)::INT AS cnt
                FROM evaluation_variant_results
                GROUP BY chunk_set_id
            )
            UPDATE chunk_sets cs
            SET indexing_refs = COALESCE(li.cnt, 0),
                variant_result_refs = COALESCE(lv.cnt, 0)
            FROM (SELECT chunk_set_id FROM chunk_sets) ids
            LEFT JOIN live_indexing li USING (chunk_set_id)
            LEFT JOIN live_variants lv USING (chunk_set_id)
            WHERE cs.chunk_set_id = ids.chunk_set_id",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("reconcile_referrer_counts: {e}"))
        })?;
        Ok(())
    }

    async fn list_unused_older_than(
        &self,
        older_than_seconds: u64,
    ) -> Result<Vec<Uuid>, ChunkSetRepositoryError> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            "SELECT chunk_set_id FROM chunk_sets
             WHERE NOT pinned
               AND indexing_refs = 0
               AND variant_result_refs = 0
               AND created_at < NOW() - make_interval(secs => $1)",
        )
        .bind(older_than_seconds as f64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("list_unused_older_than: {e}")))?;
        Ok(rows)
    }
}

fn read_facet(row: &PgRow, col: &str) -> Result<u64, ChunkSetRepositoryError> {
    let value: i64 = row
        .try_get(col)
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("facet {col}: {e}")))?;
    Ok(value.max(0) as u64)
}

fn push_status_filter(qb: &mut QueryBuilder<'_, Postgres>, filters: &[ChunkSetStatusFilter]) {
    if filters.is_empty() {
        return;
    }
    qb.push(" AND (");
    let mut first = true;
    for filter in filters {
        if !first {
            qb.push(" OR ");
        }
        first = false;
        match filter {
            ChunkSetStatusFilter::Pinned => {
                qb.push("pinned");
            }
            ChunkSetStatusFilter::Indexed => {
                qb.push("indexing_refs > 0");
            }
            ChunkSetStatusFilter::UsedByEval => {
                qb.push("variant_result_refs > 0");
            }
            ChunkSetStatusFilter::Unused => {
                qb.push("(NOT pinned AND indexing_refs = 0 AND variant_result_refs = 0)");
            }
        }
    }
    qb.push(")");
}

struct ChunkSetRow {
    chunk_set_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    chunking_config: serde_json::Value,
    chunk_count: i32,
    pinned: bool,
    indexing_refs: i32,
    variant_result_refs: i32,
    created_at: OffsetDateTime,
}

impl sqlx::FromRow<'_, PgRow> for ChunkSetRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            chunk_set_id: row.try_get("chunk_set_id")?,
            document_id: row.try_get("document_id")?,
            document_version: row.try_get("document_version")?,
            chunking_config: row.try_get("chunking_config")?,
            chunk_count: row.try_get("chunk_count")?,
            pinned: row.try_get("pinned")?,
            indexing_refs: row.try_get("indexing_refs")?,
            variant_result_refs: row.try_get("variant_result_refs")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl TryFrom<ChunkSetRow> for ChunkSetReadModel {
    type Error = ChunkSetRepositoryError;

    fn try_from(row: ChunkSetRow) -> Result<Self, Self::Error> {
        let chunking_config = serde_json::from_value(row.chunking_config).map_err(|e| {
            ChunkSetRepositoryError::Internal(format!("deserialize chunking_config: {e}"))
        })?;
        Ok(ChunkSetReadModel {
            chunk_set_id: row.chunk_set_id,
            document_id: row.document_id,
            document_version: row.document_version.cast_unsigned(),
            chunking_config,
            created_at: row.created_at.format(&Rfc3339).unwrap_or_default(),
            pinned: row.pinned,
            chunk_count: row.chunk_count.max(0) as u32,
            indexing_refs: row.indexing_refs.max(0) as u32,
            variant_result_refs: row.variant_result_refs.max(0) as u32,
        })
    }
}

struct ChunkRow {
    chunk_id: Uuid,
    sequence: i32,
    heading: String,
    text: String,
    char_start: i32,
    char_end: i32,
}

impl sqlx::FromRow<'_, PgRow> for ChunkRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            chunk_id: row.try_get("chunk_id")?,
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
            sequence: row.sequence.cast_unsigned(),
            heading: row.heading,
            text: row.text,
            char_start: row.char_start.cast_unsigned(),
            char_end: row.char_end.cast_unsigned(),
        }
    }
}
