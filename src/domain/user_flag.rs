use crate::prelude::*;
use std::collections::HashSet;

/// An admin's "keep an eye on this account" marker. Informational only — it
/// never changes what the account can do — and only admins see it.
#[derive(Debug, Clone)]
pub struct UserFlag {
    /// `None` once the flagging admin's account has been deleted.
    pub flagged_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserFlag {
    pub async fn find<'e, E>(exec: E, user_id: PrimaryKey) -> Result<Option<UserFlag>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let flag = sqlx::query_as!(
            UserFlag,
            r#"
            SELECT a.full_name AS "flagged_by_name?", f.created_at
            FROM user_flags f
            LEFT JOIN users a ON a.id = f.flagged_by
            WHERE f.user_id = $1
            "#,
            user_id
        )
        .fetch_optional(exec)
        .await?;
        Ok(flag)
    }

    /// Flagging an already-flagged account keeps the original flag.
    pub async fn flag<'e, E>(exec: E, user_id: PrimaryKey, flagged_by: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            "INSERT INTO user_flags (user_id, flagged_by) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            user_id,
            flagged_by
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn unflag<'e, E>(exec: E, user_id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!("DELETE FROM user_flags WHERE user_id = $1", user_id)
            .execute(exec)
            .await?;
        Ok(())
    }

    /// Which of `user_ids` are flagged.
    pub async fn flagged_among<'e, E>(
        exec: E,
        user_ids: &[PrimaryKey],
    ) -> Result<HashSet<PrimaryKey>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let ids = sqlx::query_scalar!(
            "SELECT user_id FROM user_flags WHERE user_id = ANY($1)",
            user_ids
        )
        .fetch_all(exec)
        .await?;
        Ok(ids.into_iter().collect())
    }
}
