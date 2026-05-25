use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::server::domain::chunk_set::entity::{Chunk, ChunkSet};
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
            "
            INSERT INTO chunk_sets (chunk_set_id, document_id, document_version, chunking_config, created_at, pinned)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (chunk_set_id) DO NOTHING
            ",
        )
        .bind(chunk_set.chunk_set_id)
        .bind(chunk_set.document_id)
        .bind(chunk_set.document_version.cast_signed())
        .bind(&chunking_config)
        .bind(&chunk_set.created_at)
        .bind(chunk_set.pinned)
        .execute(&mut *tx)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("save chunk_set: {e}")))?;

        for chunk in &chunks {
            sqlx::query(
                "
                INSERT INTO chunks (chunk_id, chunk_set_id, sequence, heading, text, char_start, char_end)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (chunk_id) DO NOTHING
                ",
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
            "SELECT chunk_set_id, document_id, document_version, chunking_config, created_at, pinned FROM chunk_sets WHERE chunk_set_id = $1",
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
            "SELECT chunk_set_id, document_id, document_version, chunking_config, created_at, pinned FROM chunk_sets WHERE document_id = $1 ORDER BY created_at DESC",
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("list chunk_sets: {e}")))?;

        rows.into_iter().map(ChunkSet::try_from).collect()
    }

    async fn delete(&self, chunk_set_id: Uuid) -> Result<(), ChunkSetRepositoryError> {
        match sqlx::query("DELETE FROM chunk_sets WHERE chunk_set_id = $1 AND NOT pinned")
            .bind(chunk_set_id)
            .execute(&self.pool)
            .await
        {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db)) if db.is_foreign_key_violation() => {
                Err(ChunkSetRepositoryError::InUse(chunk_set_id))
            }
            Err(e) => Err(ChunkSetRepositoryError::Internal(format!(
                "delete chunk_set: {e}"
            ))),
        }
    }

    async fn set_pinned(
        &self,
        chunk_set_id: Uuid,
        pinned: bool,
    ) -> Result<(), ChunkSetRepositoryError> {
        sqlx::query("UPDATE chunk_sets SET pinned = $2 WHERE chunk_set_id = $1")
            .bind(chunk_set_id)
            .bind(pinned)
            .execute(&self.pool)
            .await
            .map_err(|e| ChunkSetRepositoryError::Internal(format!("set_pinned: {e}")))?;
        Ok(())
    }

    async fn list_all_with_referrers(
        &self,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkSetReferrerRow> = sqlx::query_as(LIST_WITH_REFERRERS_SQL)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("list_all_with_referrers: {e}"))
            })?;
        rows.into_iter().map(ChunkSetReadModel::try_from).collect()
    }

    async fn list_for_document_with_referrers(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChunkSetReadModel>, ChunkSetRepositoryError> {
        let rows: Vec<ChunkSetReferrerRow> = sqlx::query_as(LIST_WITH_REFERRERS_FOR_DOC_SQL)
            .bind(document_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("list_for_document_with_referrers: {e}"))
            })?;
        rows.into_iter().map(ChunkSetReadModel::try_from).collect()
    }

    async fn delete_unused(&self, older_than_seconds: u64) -> Result<u64, ChunkSetRepositoryError> {
        let result = sqlx::query(
            "
            DELETE FROM chunk_sets cs
            WHERE NOT cs.pinned
              AND cs.created_at::timestamptz < NOW() - make_interval(secs => $1)
              AND NOT EXISTS (
                  SELECT 1 FROM indexings i
                  WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed
              )
              AND NOT EXISTS (
                  SELECT 1 FROM evaluation_variant_results r
                  WHERE r.chunk_set_id = cs.chunk_set_id
              )
            ",
        )
        .bind(older_than_seconds as f64)
        .execute(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("delete_unused: {e}")))?;
        Ok(result.rows_affected())
    }

    async fn list_page_with_referrers(
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
                QueryBuilder::new("SELECT COUNT(*)::BIGINT FROM chunk_sets cs WHERE 1=1");
            push_status_filter(&mut qb, &query.statuses);
            qb.build_query_scalar::<i64>()
                .fetch_one(&self.pool)
                .await
                .map_err(|e| ChunkSetRepositoryError::Internal(format!("count_matching: {e}")))?
        };

        let facet_row = sqlx::query(
            "
            SELECT
                SUM(CASE WHEN cs.pinned THEN 1 ELSE 0 END)::BIGINT AS pinned,
                SUM(CASE WHEN EXISTS (
                    SELECT 1 FROM indexings i
                    WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed
                ) THEN 1 ELSE 0 END)::BIGINT AS indexed,
                SUM(CASE WHEN EXISTS (
                    SELECT 1 FROM evaluation_variant_results r
                    WHERE r.chunk_set_id = cs.chunk_set_id
                ) THEN 1 ELSE 0 END)::BIGINT AS used_by_eval,
                SUM(CASE WHEN NOT cs.pinned
                    AND NOT EXISTS (
                        SELECT 1 FROM indexings i
                        WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed
                    )
                    AND NOT EXISTS (
                        SELECT 1 FROM evaluation_variant_results r
                        WHERE r.chunk_set_id = cs.chunk_set_id
                    )
                THEN 1 ELSE 0 END)::BIGINT AS unused
            FROM chunk_sets cs
            ",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ChunkSetRepositoryError::Internal(format!("status_counts: {e}")))?;

        let pinned: Option<i64> = facet_row.try_get("pinned").ok();
        let indexed: Option<i64> = facet_row.try_get("indexed").ok();
        let used_by_eval: Option<i64> = facet_row.try_get("used_by_eval").ok();
        let unused: Option<i64> = facet_row.try_get("unused").ok();
        let status_counts: Vec<(ChunkSetStatusFilter, u64)> = vec![
            (
                ChunkSetStatusFilter::Pinned,
                pinned.unwrap_or(0).max(0) as u64,
            ),
            (
                ChunkSetStatusFilter::Indexed,
                indexed.unwrap_or(0).max(0) as u64,
            ),
            (
                ChunkSetStatusFilter::UsedByEval,
                used_by_eval.unwrap_or(0).max(0) as u64,
            ),
            (
                ChunkSetStatusFilter::Unused,
                unused.unwrap_or(0).max(0) as u64,
            ),
        ];

        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "
            SELECT
                cs.chunk_set_id,
                cs.document_id,
                cs.document_version,
                cs.chunking_config,
                cs.created_at,
                cs.pinned,
                (SELECT COUNT(*)::BIGINT FROM chunks c WHERE c.chunk_set_id = cs.chunk_set_id) AS chunk_count,
                (SELECT COUNT(*)::BIGINT FROM indexings i
                    WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed) AS indexing_refs,
                (SELECT COUNT(*)::BIGINT FROM evaluation_variant_results r
                    WHERE r.chunk_set_id = cs.chunk_set_id) AS variant_result_refs
            FROM chunk_sets cs
            WHERE 1=1",
        );
        push_status_filter(&mut qb, &query.statuses);

        if let Some(cursor) = &query.cursor {
            let parsed = time::OffsetDateTime::parse(&cursor.created_at, &Rfc3339)
                .map_err(|e| ChunkSetRepositoryError::Internal(format!("cursor timestamp: {e}")))?;
            qb.push(" AND (cs.created_at::timestamptz, cs.chunk_set_id) < (");
            qb.push_bind(parsed);
            qb.push(", ");
            qb.push_bind(cursor.chunk_set_id);
            qb.push(")");
        }
        qb.push(" ORDER BY cs.created_at::timestamptz DESC, cs.chunk_set_id DESC LIMIT ");
        qb.push_bind(fetch_limit);

        let rows: Vec<ChunkSetReferrerRow> = qb
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
            items.last().map(|m| ChunkSetListCursor {
                created_at: m.created_at.clone(),
                chunk_set_id: m.chunk_set_id,
            })
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
                qb.push("cs.pinned");
            }
            ChunkSetStatusFilter::Indexed => {
                qb.push(
                    "EXISTS (SELECT 1 FROM indexings i \
                     WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed)",
                );
            }
            ChunkSetStatusFilter::UsedByEval => {
                qb.push(
                    "EXISTS (SELECT 1 FROM evaluation_variant_results r \
                     WHERE r.chunk_set_id = cs.chunk_set_id)",
                );
            }
            ChunkSetStatusFilter::Unused => {
                qb.push(
                    "(NOT cs.pinned \
                     AND NOT EXISTS (SELECT 1 FROM indexings i \
                        WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed) \
                     AND NOT EXISTS (SELECT 1 FROM evaluation_variant_results r \
                        WHERE r.chunk_set_id = cs.chunk_set_id))",
                );
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
    created_at: String,
    pinned: bool,
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
            pinned: row.try_get("pinned")?,
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
            pinned: row.pinned,
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

const LIST_WITH_REFERRERS_SQL: &str = "
    SELECT
        cs.chunk_set_id,
        cs.document_id,
        cs.document_version,
        cs.chunking_config,
        cs.created_at,
        cs.pinned,
        (SELECT COUNT(*)::BIGINT FROM chunks c WHERE c.chunk_set_id = cs.chunk_set_id) AS chunk_count,
        (SELECT COUNT(*)::BIGINT FROM indexings i
            WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed) AS indexing_refs,
        (SELECT COUNT(*)::BIGINT FROM evaluation_variant_results r
            WHERE r.chunk_set_id = cs.chunk_set_id) AS variant_result_refs
    FROM chunk_sets cs
    ORDER BY cs.created_at DESC
";

const LIST_WITH_REFERRERS_FOR_DOC_SQL: &str = "
    SELECT
        cs.chunk_set_id,
        cs.document_id,
        cs.document_version,
        cs.chunking_config,
        cs.created_at,
        cs.pinned,
        (SELECT COUNT(*)::BIGINT FROM chunks c WHERE c.chunk_set_id = cs.chunk_set_id) AS chunk_count,
        (SELECT COUNT(*)::BIGINT FROM indexings i
            WHERE i.chunk_set_id = cs.chunk_set_id AND NOT i.removed) AS indexing_refs,
        (SELECT COUNT(*)::BIGINT FROM evaluation_variant_results r
            WHERE r.chunk_set_id = cs.chunk_set_id) AS variant_result_refs
    FROM chunk_sets cs
    WHERE cs.document_id = $1
    ORDER BY cs.created_at DESC
";

struct ChunkSetReferrerRow {
    chunk_set_id: Uuid,
    document_id: Uuid,
    document_version: i32,
    chunking_config: serde_json::Value,
    created_at: String,
    pinned: bool,
    chunk_count: i64,
    indexing_refs: i64,
    variant_result_refs: i64,
}

impl sqlx::FromRow<'_, PgRow> for ChunkSetReferrerRow {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row as _;
        Ok(Self {
            chunk_set_id: row.try_get("chunk_set_id")?,
            document_id: row.try_get("document_id")?,
            document_version: row.try_get("document_version")?,
            chunking_config: row.try_get("chunking_config")?,
            created_at: row.try_get("created_at")?,
            pinned: row.try_get("pinned")?,
            chunk_count: row.try_get("chunk_count")?,
            indexing_refs: row.try_get("indexing_refs")?,
            variant_result_refs: row.try_get("variant_result_refs")?,
        })
    }
}

impl TryFrom<ChunkSetReferrerRow> for ChunkSetReadModel {
    type Error = ChunkSetRepositoryError;

    fn try_from(row: ChunkSetReferrerRow) -> Result<Self, Self::Error> {
        Ok(ChunkSetReadModel {
            chunk_set_id: row.chunk_set_id,
            document_id: row.document_id,
            document_version: row.document_version.cast_unsigned(),
            chunking_config: serde_json::from_value(row.chunking_config).map_err(|e| {
                ChunkSetRepositoryError::Internal(format!("deserialize chunking_config: {e}"))
            })?,
            created_at: row.created_at,
            pinned: row.pinned,
            chunk_count: row.chunk_count.max(0) as u32,
            indexing_refs: row.indexing_refs.max(0) as u32,
            variant_result_refs: row.variant_result_refs.max(0) as u32,
        })
    }
}
