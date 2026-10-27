use crate::prelude::*;
use chrono::NaiveDate;

/// How many trailing UTC days the admin user page's activity strip covers.
pub const ACTIVITY_WINDOW_DAYS: i32 = 30;

/// One day in a user's activity strip: whether they made an authenticated
/// request that (UTC) day, per `user_daily_activity`.
#[derive(Debug, Clone, FromRow)]
pub struct ActiveDay {
    pub day: NaiveDate,
    pub active: bool,
}

impl ActiveDay {
    /// One entry per day in the window, oldest first. Days are UTC to match
    /// how `User::audit_and_find_by_email` records them.
    pub async fn recent_for_user<'e, E>(exec: E, user_id: PrimaryKey) -> Result<Vec<ActiveDay>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let rows = sqlx::query_as!(
            ActiveDay,
            r#"
            SELECT d.day::DATE AS "day!",
                   (uda.user_id IS NOT NULL) AS "active!"
            FROM generate_series(
                     (NOW() AT TIME ZONE 'UTC')::DATE - ($2::INT - 1),
                     (NOW() AT TIME ZONE 'UTC')::DATE,
                     INTERVAL '1 day'
                 ) AS d(day)
            LEFT JOIN user_daily_activity uda
                   ON uda.user_id = $1 AND uda.day = d.day::DATE
            ORDER BY d.day
            "#,
            user_id,
            ACTIVITY_WINDOW_DAYS
        )
        .fetch_all(exec)
        .await?;
        Ok(rows)
    }
}
