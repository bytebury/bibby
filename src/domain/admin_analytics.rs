use crate::prelude::*;
use chrono::NaiveDate;
use num_format::{Locale, ToFormattedString};
use serde_json::json;
use std::collections::HashMap;

/// Site-wide snapshot powering the admin analytics dashboard.
///
/// Each section is its own query so a fresh database still renders cleanly.
#[derive(Debug, Default)]
pub struct AdminAnalytics {
    pub total_users: i64,
    pub free_users: i64,
    pub pro_users: i64,
    pub new_users_last_30d: i64,
    pub new_users_prev_30d: i64,
    /// Distinct users with at least one authenticated request in the window.
    pub active_users_last_30d: i64,
    pub active_users_prev_30d: i64,

    /// 30 entries, oldest first. `current_window[29]` is today.
    pub current_window: Vec<DailyActiveSlice>,
    /// 30 entries, oldest first, covering the 30 days immediately before the
    /// current window. Same length as `current_window` for direct overlay.
    pub previous_window: Vec<DailyActiveSlice>,
    pub current_window_total: i64,
    pub previous_window_total: i64,

    pub top_countries: Vec<CountrySlice>,
}

#[derive(Debug, Clone)]
pub struct DailyActiveSlice {
    pub day: NaiveDate,
    pub active_users: i64,
}

#[derive(Debug, Clone)]
pub struct CountrySlice {
    pub name: String,
    pub code: String,
    pub user_count: i64,
}

/// A current-period value compared against the prior period. The template
/// uses [`Delta::formatted`] and [`Delta::direction`] to render a coloured
/// percentage badge in the top-right of each KPI card.
#[derive(Debug, Clone, Copy, Default)]
pub struct Delta {
    pub current: i64,
    pub previous: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Flat,
}

impl Delta {
    pub fn new(current: i64, previous: i64) -> Self {
        Self { current, previous }
    }

    pub fn direction(&self) -> Direction {
        match self.current.cmp(&self.previous) {
            std::cmp::Ordering::Greater => Direction::Up,
            std::cmp::Ordering::Less => Direction::Down,
            std::cmp::Ordering::Equal => Direction::Flat,
        }
    }

    pub fn is_up(&self) -> bool {
        self.direction() == Direction::Up
    }

    pub fn is_down(&self) -> bool {
        self.direction() == Direction::Down
    }

    /// `+12.3%` / `-5.0%` string for templates. Returns `None` only when
    /// both periods are zero (no signal to display).
    pub fn formatted(&self) -> Option<String> {
        if self.previous == 0 {
            if self.current == 0 {
                return None;
            }
            // No prior baseline but we have current activity — show "new"
            // instead of dividing by zero.
            return Some("new".to_string());
        }
        let pct = (self.current - self.previous) as f64 / self.previous as f64 * 100.0;
        let sign = if pct > 0.0 { "+" } else { "" };
        Some(format!("{sign}{pct:.1}%"))
    }
}

/// Thousands separators for KPI numbers (e.g. `2,689`).
pub fn format_int(value: i64) -> String {
    value.to_formatted_string(&Locale::en)
}

impl AdminAnalytics {
    /// Pre-serialized JSON consumed by the page's Chart.js. Keeps user-supplied
    /// strings out of inline JS literals.
    pub fn chart_json(&self) -> String {
        crate::infra::api::script_json(&json!({
            "dau": {
                "labels": self.current_window.iter().map(|d| d.day.to_string()).collect::<Vec<_>>(),
                "current": self.current_window.iter().map(|d| d.active_users).collect::<Vec<_>>(),
                "previousLabels": self.previous_window.iter().map(|d| d.day.to_string()).collect::<Vec<_>>(),
                "previous": self.previous_window.iter().map(|d| d.active_users).collect::<Vec<_>>(),
            },
        }))
    }

    pub fn users_delta(&self) -> Delta {
        Delta::new(self.new_users_last_30d, self.new_users_prev_30d)
    }
    pub fn active_users_delta(&self) -> Delta {
        Delta::new(self.active_users_last_30d, self.active_users_prev_30d)
    }
    pub fn dau_delta(&self) -> Delta {
        Delta::new(self.current_window_total, self.previous_window_total)
    }

    pub async fn load<'e, E>(exec: E) -> Self
    where
        E: Executor<'e, Database = sqlx::Postgres> + Copy,
    {
        // The four queries are independent, so they run at once.
        let users = sqlx::query!(
            r#"
            SELECT
                COUNT(*)                                         AS "total!",
                COUNT(*) FILTER (WHERE role = 'free')            AS "free!",
                COUNT(*) FILTER (WHERE role = 'professional')    AS "pro!",
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '30 days')                                                AS "new_30d!",
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '60 days' AND created_at < NOW() - INTERVAL '30 days')   AS "new_prev_30d!"
            FROM users
            "#,
        )
        .fetch_one(exec);

        let active = sqlx::query!(
            r#"
            SELECT
                COUNT(DISTINCT user_id) FILTER (WHERE day > (NOW() AT TIME ZONE 'UTC')::DATE - 30)                                                  AS "last_30d!",
                COUNT(DISTINCT user_id) FILTER (WHERE day <= (NOW() AT TIME ZONE 'UTC')::DATE - 30 AND day > (NOW() AT TIME ZONE 'UTC')::DATE - 60) AS "prev_30d!"
            FROM user_daily_activity
            "#,
        )
        .fetch_one(exec);

        // Active users per day, sourced from `user_daily_activity` — a row is
        // written there at most once per (user, day) by the per-request
        // audit, so each row maps 1:1 to a user who was signed in that day.
        // Spans the last 60 days so it splits into current vs previous 30-day
        // windows.
        let activity = sqlx::query!(
            r#"
            SELECT day                AS "day!",
                   COUNT(*)::BIGINT   AS "active_count!"
            FROM user_daily_activity
            WHERE day >= (NOW() AT TIME ZONE 'UTC')::DATE - INTERVAL '60 days'
            GROUP BY day
            ORDER BY day
            "#,
        )
        .fetch_all(exec);

        let countries = sqlx::query!(
            r#"
            SELECT
                COALESCE(c.name, 'Unknown') AS "name!",
                COALESCE(c.code, 'unknown') AS "code!",
                COUNT(u.id)                 AS "user_count!"
            FROM users u
            LEFT JOIN countries c ON c.id = u.country_id
            -- Users with no country and those in the auto-created "unknown"
            -- country are the same thing to an admin, so group on the
            -- displayed values.
            GROUP BY 1, 2
            ORDER BY COUNT(u.id) DESC
            LIMIT 5
            "#,
        )
        .fetch_all(exec);

        let (users, active, activity, countries) = tokio::join!(users, active, activity, countries);

        let mut analytics = Self::default();
        match users {
            Ok(r) => {
                analytics.total_users = r.total;
                analytics.free_users = r.free;
                analytics.pro_users = r.pro;
                analytics.new_users_last_30d = r.new_30d;
                analytics.new_users_prev_30d = r.new_prev_30d;
            }
            Err(error) => warn!(%error, "could not load user analytics"),
        }

        match active {
            Ok(r) => {
                analytics.active_users_last_30d = r.last_30d;
                analytics.active_users_prev_30d = r.prev_30d;
            }
            Err(error) => warn!(%error, "could not load active-user analytics"),
        }

        let mut buckets: HashMap<NaiveDate, i64> = activity
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.day, r.active_count))
            .collect();
        let today = Utc::now().date_naive();
        let mut window = |offsets: std::ops::Range<i64>| -> Vec<DailyActiveSlice> {
            offsets
                .rev()
                .map(|offset| {
                    let day = today - Duration::days(offset);
                    DailyActiveSlice {
                        day,
                        active_users: buckets.remove(&day).unwrap_or(0),
                    }
                })
                .collect()
        };
        analytics.current_window = window(0..30);
        analytics.previous_window = window(30..60);
        analytics.current_window_total = analytics
            .current_window
            .iter()
            .map(|s| s.active_users)
            .sum();
        analytics.previous_window_total = analytics
            .previous_window
            .iter()
            .map(|s| s.active_users)
            .sum();

        analytics.top_countries = countries
            .unwrap_or_default()
            .into_iter()
            .map(|r| CountrySlice {
                name: r.name,
                code: r.code,
                user_count: r.user_count,
            })
            .collect();
        analytics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_compare_the_two_periods() {
        assert_eq!(Delta::new(12, 10).formatted().as_deref(), Some("+20.0%"));
        assert_eq!(Delta::new(5, 10).formatted().as_deref(), Some("-50.0%"));
        assert_eq!(Delta::new(10, 10).formatted().as_deref(), Some("0.0%"));
        assert!(Delta::new(12, 10).is_up());
        assert!(Delta::new(5, 10).is_down());
    }

    #[test]
    fn deltas_without_a_baseline() {
        assert_eq!(Delta::new(3, 0).formatted().as_deref(), Some("new"));
        assert_eq!(Delta::new(0, 0).formatted(), None);
    }

    #[test]
    fn formats_thousands() {
        assert_eq!(format_int(2689), "2,689");
        assert_eq!(format_int(7), "7");
    }

    #[test]
    fn chart_json_cannot_close_the_script_tag() {
        let analytics = AdminAnalytics::default();
        assert!(!analytics.chart_json().contains("</"));
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use crate::domain::user::Role;
    use crate::test_support::insert_user;
    use sqlx::PgPool;

    #[sqlx::test]
    async fn total_users_splits_into_free_and_pro(db: PgPool) {
        insert_user(&db, "free1@example.com", Role::Free, "10.0.3.1").await;
        insert_user(&db, "free2@example.com", Role::Free, "10.0.3.1").await;
        insert_user(&db, "pro@example.com", Role::Professional, "10.0.3.1").await;
        insert_user(&db, "admin@example.com", Role::Admin, "10.0.3.1").await;

        let analytics = AdminAnalytics::load(&db).await;
        assert_eq!(analytics.total_users, 4);
        assert_eq!((analytics.free_users, analytics.pro_users), (2, 1));
        assert_eq!(analytics.new_users_last_30d, 4);
    }
}
