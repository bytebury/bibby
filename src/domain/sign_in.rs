use crate::domain::user::User;
use crate::infra::pagination::Paginated;
use crate::prelude::*;

/// How long sign-in history is kept. Each user's most recent sign-in is kept
/// past this no matter how old it is.
pub const SIGN_IN_RETENTION_DAYS: i32 = 30;

/// One row in `user_sign_ins`: a successful sign-in and where it came from.
pub struct SignIn;

/// A user's sign-ins grouped by IP and the location it resolved to — what the
/// admin user page lists.
#[derive(Debug, Clone, FromRow)]
pub struct SignInLocation {
    pub ip: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub region_name: Option<String>,
    pub city_name: Option<String>,
    pub sign_in_count: i64,
    pub last_signed_in_at: DateTime<Utc>,
}

impl SignInLocation {
    /// "City, Region, Country", skipping whatever is unknown.
    pub fn place(&self) -> String {
        [&self.city_name, &self.region_name, &self.country_name]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Another account that has used one of this user's IPs — a hint that they
/// may be the same person.
pub struct SharedIpAccount {
    pub user: User,
    pub shared_ips: Vec<String>,
}

impl SignIn {
    pub async fn record<'e, E>(
        exec: E,
        user_id: PrimaryKey,
        ip: &str,
        country_id: Option<PrimaryKey>,
        region_id: Option<PrimaryKey>,
        city_id: Option<PrimaryKey>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            r#"
            INSERT INTO user_sign_ins (user_id, ip, country_id, region_id, city_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            user_id,
            ip,
            country_id,
            region_id,
            city_id
        )
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Sign-ins inside the retention window, plus the latest one even if it's
    /// older, grouped by IP + location, most recent first, one page at a time.
    pub async fn locations_for_user(
        db: &sqlx::PgPool,
        user_id: PrimaryKey,
        page: i64,
        page_size: i64,
    ) -> Result<Paginated<SignInLocation>> {
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!" FROM (
                SELECT 1
                FROM user_sign_ins s
                WHERE s.user_id = $1
                  AND (s.created_at >= NOW() - make_interval(days => $2)
                       OR s.id = (SELECT MAX(id) FROM user_sign_ins WHERE user_id = $1))
                GROUP BY s.ip, s.country_id, s.region_id, s.city_id
            ) grouped
            "#,
            user_id,
            SIGN_IN_RETENTION_DAYS
        )
        .fetch_one(db)
        .await?;

        // Past the end (e.g. a stale link) lands on the last page instead.
        let last_page = ((total + page_size - 1) / page_size).max(1);
        let page = page.clamp(1, last_page);

        let rows = sqlx::query_as!(
            SignInLocation,
            r#"
            SELECT s.ip,
                   c.code AS "country_code?",
                   c.name AS "country_name?",
                   r.name AS "region_name?",
                   ci.name AS "city_name?",
                   COUNT(*) AS "sign_in_count!",
                   MAX(s.created_at) AS "last_signed_in_at!"
            FROM user_sign_ins s
            LEFT JOIN countries c ON c.id = s.country_id
            LEFT JOIN regions r ON r.id = s.region_id
            LEFT JOIN cities ci ON ci.id = s.city_id
            WHERE s.user_id = $1
              AND (s.created_at >= NOW() - make_interval(days => $2)
                   OR s.id = (SELECT MAX(id) FROM user_sign_ins WHERE user_id = $1))
            GROUP BY s.ip, s.country_id, s.region_id, s.city_id, c.code, c.name, r.name, ci.name
            ORDER BY MAX(s.created_at) DESC, s.ip
            LIMIT $3 OFFSET $4
            "#,
            user_id,
            SIGN_IN_RETENTION_DAYS,
            page_size,
            (page - 1) * page_size
        )
        .fetch_all(db)
        .await?;
        Ok(Paginated::new(rows, total, page, page_size))
    }

    /// Other accounts that signed in from (or were last seen on) any IP this
    /// user has, excluding accounts already linked to them. Newest accounts
    /// first, capped at `limit`; the second value is the uncapped total.
    pub async fn accounts_sharing_ips(
        db: &sqlx::PgPool,
        user_id: PrimaryKey,
        limit: i64,
    ) -> Result<(Vec<SharedIpAccount>, i64)> {
        let rows = sqlx::query!(
            r#"
            WITH mine AS (
                SELECT ip FROM user_sign_ins WHERE user_id = $1
                UNION
                SELECT last_known_ip FROM users WHERE id = $1
            ),
            linked AS (
                SELECT other.user_id
                FROM account_group_members me
                JOIN account_group_members other ON other.group_id = me.group_id
                WHERE me.user_id = $1
            ),
            matches AS (
                SELECT s.user_id, s.ip
                FROM user_sign_ins s
                JOIN mine m ON m.ip = s.ip
                UNION
                SELECT u.id, u.last_known_ip
                FROM users u
                JOIN mine m ON m.ip = u.last_known_ip
            )
            SELECT user_id AS "user_id!",
                   array_agg(DISTINCT ip ORDER BY ip) AS "shared_ips!",
                   COUNT(*) OVER () AS "total!"
            FROM matches
            WHERE user_id <> $1
              AND ip NOT IN ('', 'unknown')
              AND user_id NOT IN (SELECT user_id FROM linked)
            GROUP BY user_id
            ORDER BY user_id DESC
            LIMIT $2
            "#,
            user_id,
            limit
        )
        .fetch_all(db)
        .await?;

        let total = rows.first().map(|r| r.total).unwrap_or(0);
        let ids: Vec<PrimaryKey> = rows.iter().map(|r| r.user_id).collect();
        let users: Vec<User> = sqlx::query_as("SELECT * FROM user_details WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(db)
            .await?;

        let mut accounts: Vec<SharedIpAccount> = rows
            .into_iter()
            .filter_map(|row| {
                let user = users.iter().find(|u| u.id == row.user_id)?.clone();
                Some(SharedIpAccount {
                    user,
                    shared_ips: row.shared_ips,
                })
            })
            .collect();
        accounts.sort_by_key(|a| std::cmp::Reverse(a.user.created_at));
        Ok((accounts, total))
    }

    /// Drops sign-ins older than the retention window, keeping each user's
    /// most recent one regardless of age. Returns how many rows were removed.
    pub async fn prune<'e, E>(exec: E) -> Result<u64>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_sign_ins s
            WHERE s.created_at < NOW() - make_interval(days => $1)
              AND s.id <> (SELECT MAX(latest.id) FROM user_sign_ins latest
                           WHERE latest.user_id = s.user_id)
            "#,
            SIGN_IN_RETENTION_DAYS
        )
        .execute(exec)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(city: Option<&str>, region: Option<&str>, country: Option<&str>) -> SignInLocation {
        SignInLocation {
            ip: "127.0.0.1".to_string(),
            country_code: None,
            country_name: country.map(str::to_string),
            region_name: region.map(str::to_string),
            city_name: city.map(str::to_string),
            sign_in_count: 1,
            last_signed_in_at: Utc::now(),
        }
    }

    #[test]
    fn place_skips_unknown_parts() {
        assert_eq!(
            location(Some("Denver"), Some("Colorado"), Some("United States")).place(),
            "Denver, Colorado, United States"
        );
        assert_eq!(location(None, None, Some("Canada")).place(), "Canada");
        assert_eq!(location(None, None, None).place(), "");
    }
}
