use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::server::domain::configuration::sweep_template::{
    SweepTemplateReadModel, SweepTemplateRepository, SweepTemplateRepositoryError,
};

pub struct PostgresSweepTemplateRepository {
    pool: PgPool,
}

impl PostgresSweepTemplateRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SweepTemplateRepository for PostgresSweepTemplateRepository {
    async fn load_all(&self) -> Result<Vec<SweepTemplateReadModel>, SweepTemplateRepositoryError> {
        let rows: Vec<SweepTemplateRow> = sqlx::query_as(
            r#"
            SELECT id, name, members, is_default
            FROM sweep_templates
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SweepTemplateRepositoryError::Internal(format!("load_all: {e}")))?;

        rows.into_iter().map(SweepTemplateRow::try_into).collect()
    }

    async fn save(
        &self,
        read_model: SweepTemplateReadModel,
    ) -> Result<(), SweepTemplateRepositoryError> {
        let members_json = serde_json::to_value(&read_model.members).map_err(|e| {
            SweepTemplateRepositoryError::Internal(format!("serialize members: {e}"))
        })?;

        sqlx::query(
            r#"
            INSERT INTO sweep_templates (id, name, members)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET
                name       = $2,
                members    = $3,
                updated_at = NOW()
            "#,
        )
        .bind(read_model.sweep_template_id)
        .bind(&read_model.name)
        .bind(&members_json)
        .execute(&self.pool)
        .await
        .map_err(|e| SweepTemplateRepositoryError::Internal(format!("save: {e}")))?;

        Ok(())
    }

    async fn set_default(&self, id: Uuid) -> Result<(), SweepTemplateRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            SweepTemplateRepositoryError::Internal(format!("set_default begin: {e}"))
        })?;

        sqlx::query("UPDATE sweep_templates SET is_default = FALSE WHERE is_default")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                SweepTemplateRepositoryError::Internal(format!("set_default clear: {e}"))
            })?;

        sqlx::query("UPDATE sweep_templates SET is_default = TRUE WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| SweepTemplateRepositoryError::Internal(format!("set_default set: {e}")))?;

        tx.commit().await.map_err(|e| {
            SweepTemplateRepositoryError::Internal(format!("set_default commit: {e}"))
        })?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), SweepTemplateRepositoryError> {
        sqlx::query("DELETE FROM sweep_templates WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| SweepTemplateRepositoryError::Internal(format!("delete: {e}")))?;

        Ok(())
    }
}

struct SweepTemplateRow {
    id: Uuid,
    name: String,
    members: serde_json::Value,
    is_default: bool,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for SweepTemplateRow {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            members: row.try_get("members")?,
            is_default: row.try_get("is_default")?,
        })
    }
}

impl TryFrom<SweepTemplateRow> for SweepTemplateReadModel {
    type Error = SweepTemplateRepositoryError;

    fn try_from(row: SweepTemplateRow) -> Result<Self, Self::Error> {
        let members: Vec<Uuid> = serde_json::from_value(row.members).map_err(|e| {
            SweepTemplateRepositoryError::Internal(format!("deserialize members: {e}"))
        })?;
        Ok(Self {
            sweep_template_id: row.id,
            name: row.name,
            members,
            is_default: row.is_default,
        })
    }
}
