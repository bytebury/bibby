use crate::domain::user::User;
use crate::prelude::*;
use std::collections::HashMap;

/// Admin-curated groups of accounts that belong to the same person. A user is
/// in at most one group (`account_group_members.user_id` is the primary key).
pub struct AccountGroup;

impl AccountGroup {
    /// The other accounts linked to `user_id`, by name.
    pub async fn linked_accounts<'e, E>(exec: E, user_id: PrimaryKey) -> Result<Vec<User>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let users = sqlx::query_as(
            "SELECT ud.*
             FROM account_group_members me
             JOIN account_group_members other
                  ON other.group_id = me.group_id AND other.user_id <> me.user_id
             JOIN user_details ud ON ud.id = other.user_id
             WHERE me.user_id = $1
             ORDER BY LOWER(ud.full_name), ud.id",
        )
        .bind(user_id)
        .fetch_all(exec)
        .await?;
        Ok(users)
    }

    /// For each of `user_ids` that is linked, how many *other* accounts it is
    /// linked to. Unlinked users are absent from the map.
    pub async fn linked_counts<'e, E>(
        exec: E,
        user_ids: &[PrimaryKey],
    ) -> Result<HashMap<PrimaryKey, i64>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let rows = sqlx::query!(
            r#"
            SELECT me.user_id, COUNT(*) - 1 AS "others!"
            FROM account_group_members me
            JOIN account_group_members everyone ON everyone.group_id = me.group_id
            WHERE me.user_id = ANY($1)
            GROUP BY me.user_id
            "#,
            user_ids
        )
        .fetch_all(exec)
        .await?;
        Ok(rows.into_iter().map(|r| (r.user_id, r.others)).collect())
    }

    /// Puts `a` and `b` in the same group, creating one or merging two as
    /// needed. Run inside a transaction.
    pub async fn link(
        conn: &mut sqlx::PgConnection,
        a: PrimaryKey,
        b: PrimaryKey,
        linked_by: PrimaryKey,
    ) -> Result<()> {
        let rows = sqlx::query!(
            "SELECT user_id, group_id FROM account_group_members
             WHERE user_id = ANY($1) FOR UPDATE",
            &[a, b][..]
        )
        .fetch_all(&mut *conn)
        .await?;
        let group_of = |id| rows.iter().find(|r| r.user_id == id).map(|r| r.group_id);

        match (group_of(a), group_of(b)) {
            (Some(ga), Some(gb)) if ga == gb => {}
            (Some(ga), Some(gb)) => {
                sqlx::query!(
                    "UPDATE account_group_members SET group_id = $1 WHERE group_id = $2",
                    ga,
                    gb
                )
                .execute(&mut *conn)
                .await?;
                sqlx::query!("DELETE FROM account_groups WHERE id = $1", gb)
                    .execute(&mut *conn)
                    .await?;
            }
            (Some(group), None) => Self::add_member(conn, group, b, linked_by).await?,
            (None, Some(group)) => Self::add_member(conn, group, a, linked_by).await?,
            (None, None) => {
                let group =
                    sqlx::query_scalar!("INSERT INTO account_groups DEFAULT VALUES RETURNING id")
                        .fetch_one(&mut *conn)
                        .await?;
                Self::add_member(conn, group, a, linked_by).await?;
                Self::add_member(conn, group, b, linked_by).await?;
            }
        }
        Ok(())
    }

    async fn add_member(
        conn: &mut sqlx::PgConnection,
        group_id: PrimaryKey,
        user_id: PrimaryKey,
        linked_by: PrimaryKey,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT INTO account_group_members (user_id, group_id, linked_by) VALUES ($1, $2, $3)",
            user_id,
            group_id,
            linked_by
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Takes `user_id` out of its group. A group left with a single account
    /// is deleted (cascading its last membership). Run inside a transaction.
    pub async fn unlink(conn: &mut sqlx::PgConnection, user_id: PrimaryKey) -> Result<()> {
        let group = sqlx::query_scalar!(
            "DELETE FROM account_group_members WHERE user_id = $1 RETURNING group_id",
            user_id
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(group) = group {
            sqlx::query!(
                "DELETE FROM account_groups g
                 WHERE g.id = $1
                   AND (SELECT COUNT(*) FROM account_group_members m WHERE m.group_id = g.id) < 2",
                group
            )
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }
}
