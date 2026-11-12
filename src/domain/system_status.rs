//! The "state of the union" behind the admin monitoring page: is the
//! database healthy, did the daily jobs run, is the app configured, and how
//! busy has the last day been. Every section is loaded on its own so one
//! failing query (or a fresh database) still leaves the rest of the page.

use crate::infra::monitoring::{self, RequestStats};
use crate::prelude::*;
use sqlx::PgPool;
use std::time::Instant;

/// A daily tick older than this is flagged; the cron runs once a day.
const SCHEDULER_STALE_HOURS: i64 = 26;
/// A database round trip slower than this is flagged.
const SLOW_PING_MS: u64 = 250;

#[derive(Debug)]
pub struct SystemStatus {
    pub checked_at: DateTime<Utc>,
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub memory_bytes: Option<u64>,
    pub threads: Option<u64>,

    pub database: Option<DatabaseStatus>,
    pub pool: PoolStatus,
    pub tables: Vec<TableSize>,
    pub migrations: Option<MigrationStatus>,
    pub scheduler: Option<SchedulerStatus>,
    pub pulse: Option<Pulse>,
    pub config: Vec<ConfigCheck>,
    pub requests: RequestStats,
    pub log_lines: usize,
    pub log_warnings: usize,
    pub log_errors: usize,

    /// Problems worth an admin's attention, most serious first.
    pub issues: Vec<Issue>,
}

#[derive(Debug)]
pub struct DatabaseStatus {
    /// Round trip in microseconds.
    pub ping_us: u64,
    pub server_version: String,
    pub size_bytes: i64,
    pub connections: i64,
    pub max_connections: i64,
}

#[derive(Debug)]
pub struct PoolStatus {
    pub size: u32,
    pub idle: u32,
    pub max: u32,
}

#[derive(Debug)]
pub struct TableSize {
    pub name: String,
    pub rows: i64,
    pub bytes: i64,
}

#[derive(Debug)]
pub struct MigrationStatus {
    pub applied: i64,
    pub failed: i64,
    pub bundled: usize,
    pub latest_version: Option<i64>,
    pub latest_description: Option<String>,
    pub latest_installed_at: Option<DateTime<Utc>>,
}

impl DatabaseStatus {
    /// "0.4 ms", "12 ms".
    pub fn ping(&self) -> String {
        let ms = self.ping_us as f64 / 1000.0;
        if ms < 10.0 {
            format!("{ms:.1} ms")
        } else {
            format!("{ms:.0} ms")
        }
    }

    pub fn size(&self) -> String {
        format_bytes(self.size_bytes)
    }
}

impl TableSize {
    pub fn size(&self) -> String {
        format_bytes(self.bytes)
    }
}

impl MigrationStatus {
    pub fn pending(&self) -> i64 {
        (self.bundled as i64 - self.applied).max(0)
    }
}

#[derive(Debug)]
pub struct SchedulerStatus {
    /// When the daily lease last changed hands: a finished tick releases it.
    pub last_run_at: DateTime<Utc>,
    pub running: bool,
}

impl SchedulerStatus {
    pub fn ago(&self, now: DateTime<Utc>) -> String {
        humanize(now - self.last_run_at)
    }

    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        now - self.last_run_at > chrono::Duration::hours(SCHEDULER_STALE_HOURS)
    }
}

/// Activity over the last 24 hours.
#[derive(Debug)]
pub struct Pulse {
    pub new_users: i64,
    pub active_users: i64,
    pub sign_ins: i64,
    pub unverified_sign_ups: i64,
    pub pro_users: i64,
}

/// One minute's bar in the traffic chart (an SVG `TrafficBar::HEIGHT` tall).
#[derive(Debug)]
pub struct TrafficBar {
    pub x: usize,
    pub height: u32,
    pub error_height: u32,
    pub label: String,
}

impl TrafficBar {
    pub const HEIGHT: f64 = 48.0;
}

#[derive(Debug)]
pub struct ConfigCheck {
    pub name: &'static str,
    pub purpose: &'static str,
    pub set: bool,
    /// Whether the app is broken without it (vs. a feature going quiet).
    pub required: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    Warning,
}

#[derive(Debug)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

impl Issue {
    pub fn is_critical(&self) -> bool {
        self.severity == Severity::Critical
    }
}

/// The environment variables worth knowing about. Only whether each one is
/// set is ever read back — never the value.
const CONFIG: &[(&str, &str, bool)] = &[
    ("DATABASE_URL", "Postgres connection", true),
    ("JWT_SECRET", "Signs session cookies", true),
    ("APP_ORIGIN", "Absolute links in emails", true),
    ("GOOGLE_CLIENT_ID", "Google sign-in", true),
    ("GOOGLE_CLIENT_SECRET", "Google sign-in", true),
    ("GOOGLE_CALLBACK_URL", "Google sign-in", true),
    ("MAILMAN_URL", "Sign-in codes and emails", true),
    ("MAILMAN_API_TOKEN", "Sign-in codes and emails", true),
    ("STRIPE_SECRET_KEY", "Checkout and billing", true),
    ("STRIPE_WEBHOOK_SECRET", "Stripe webhooks", true),
    ("CRON_SECRET", "Daily jobs (cron tick)", true),
    ("PUBLIC_SITE_URL", "Canonical URLs for SEO", false),
];

impl SystemStatus {
    pub async fn load(db: &PgPool) -> Self {
        let now = Utc::now();
        let (memory_bytes, threads) = monitoring::process_usage();
        let (log_lines, log_warnings, log_errors) = monitoring::log_counts();

        let database = Self::database(db).await;
        let tables = Self::tables(db).await;
        let migrations = Self::migrations(db).await;
        let scheduler = Self::scheduler(db).await;
        let pulse = Self::pulse(db).await;

        let config = CONFIG
            .iter()
            .map(|&(name, purpose, required)| ConfigCheck {
                name,
                purpose,
                required,
                set: std::env::var(name).is_ok_and(|v| !v.trim().is_empty()),
            })
            .collect();

        let mut status = Self {
            checked_at: now,
            version: build_version(),
            started_at: monitoring::started_at(),
            memory_bytes,
            threads,
            database,
            pool: PoolStatus {
                size: db.size(),
                idle: db.num_idle() as u32,
                max: db.options().get_max_connections(),
            },
            tables,
            migrations,
            scheduler,
            pulse,
            config,
            requests: monitoring::request_stats(),
            log_lines,
            log_warnings,
            log_errors,
            issues: Vec::new(),
        };
        status.issues = status.find_issues();
        status
    }

    pub fn uptime(&self) -> String {
        humanize(self.checked_at - self.started_at)
    }

    pub fn scheduler_ago(&self) -> String {
        self.scheduler
            .as_ref()
            .map(|s| s.ago(self.checked_at))
            .unwrap_or_default()
    }

    pub fn scheduler_is_stale(&self) -> bool {
        self.scheduler
            .as_ref()
            .is_some_and(|s| s.is_stale(self.checked_at))
    }

    pub fn short_version(&self) -> &str {
        self.version.get(..7).unwrap_or(&self.version)
    }

    pub fn memory(&self) -> Option<String> {
        self.memory_bytes.map(|b| format_bytes(b as i64))
    }

    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn has_critical(&self) -> bool {
        self.issues.iter().any(Issue::is_critical)
    }

    /// Server errors in the last hour, from the per-minute buckets.
    pub fn errors_last_hour(&self) -> u64 {
        self.requests
            .traffic(self.checked_at)
            .iter()
            .map(|b| b.errors)
            .sum()
    }

    pub fn requests_last_hour(&self) -> u64 {
        self.requests
            .traffic(self.checked_at)
            .iter()
            .map(|b| b.requests)
            .sum()
    }

    /// One bar per minute for the last hour, oldest first, scaled to the
    /// busiest minute, for the traffic chart.
    pub fn traffic_bars(&self) -> Vec<TrafficBar> {
        let traffic = self.requests.traffic(self.checked_at);
        let peak = traffic.iter().map(|b| b.requests).max().unwrap_or(0).max(1) as f64;
        let len = traffic.len();
        traffic
            .iter()
            .enumerate()
            .map(|(i, b)| TrafficBar {
                x: i,
                height: (b.requests as f64 / peak * TrafficBar::HEIGHT).round() as u32,
                error_height: (b.errors as f64 / peak * TrafficBar::HEIGHT).ceil() as u32,
                label: format!(
                    "{} min ago: {} request{}, {} server error{}",
                    len - 1 - i,
                    b.requests,
                    if b.requests == 1 { "" } else { "s" },
                    b.errors,
                    if b.errors == 1 { "" } else { "s" },
                ),
            })
            .collect()
    }

    pub fn peak_requests_per_minute(&self) -> u64 {
        self.requests
            .traffic(self.checked_at)
            .iter()
            .map(|b| b.requests)
            .max()
            .unwrap_or(0)
    }

    pub fn test_auth_enabled(&self) -> bool {
        std::env::var("ENABLE_TEST_AUTH")
            .is_ok_and(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
    }

    fn find_issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut push = |severity, message: String| issues.push(Issue { severity, message });

        match &self.database {
            None => push(Severity::Critical, "The database didn't answer.".into()),
            Some(db) if db.ping_us > SLOW_PING_MS * 1000 => push(
                Severity::Warning,
                format!("The database took {} to answer a ping.", db.ping()),
            ),
            Some(db) if db.max_connections > 0 && db.connections * 10 >= db.max_connections * 9 => {
                push(
                    Severity::Warning,
                    format!(
                        "{} of {} Postgres connections are in use.",
                        db.connections, db.max_connections
                    ),
                )
            }
            _ => {}
        }
        if let Some(m) = &self.migrations {
            if m.failed > 0 {
                push(
                    Severity::Critical,
                    format!("{} migration(s) failed to apply.", m.failed),
                );
            }
            if m.pending() > 0 {
                push(
                    Severity::Warning,
                    format!("{} migration(s) haven't been applied.", m.pending()),
                );
            }
        }
        match &self.scheduler {
            None => push(Severity::Warning, "The daily jobs have never run.".into()),
            Some(s) if s.is_stale(self.checked_at) => push(
                Severity::Critical,
                format!(
                    "The daily jobs last ran {} ago.",
                    humanize(self.checked_at - s.last_run_at)
                ),
            ),
            _ => {}
        }
        for check in self.config.iter().filter(|c| c.required && !c.set) {
            push(
                Severity::Critical,
                format!("{} isn't set ({}).", check.name, check.purpose),
            );
        }
        if self.test_auth_enabled() {
            push(
                Severity::Critical,
                "The test sign-in shim (ENABLE_TEST_AUTH) is on. Anyone with the token can sign in as anyone.".into(),
            );
        }
        let errors = self.errors_last_hour();
        if errors > 0 {
            push(
                Severity::Warning,
                format!(
                    "{errors} server error{} in the last hour.",
                    if errors == 1 { "" } else { "s" }
                ),
            );
        }
        issues.sort_by(|a, b| a.severity.cmp(&b.severity));
        issues
    }

    async fn database(db: &PgPool) -> Option<DatabaseStatus> {
        let started = Instant::now();
        sqlx::query_scalar!("SELECT 1 AS \"one!\"")
            .fetch_one(db)
            .await
            .ok()?;
        let ping_us = started.elapsed().as_micros() as u64;

        let row = sqlx::query!(
            r#"
            SELECT
                current_setting('server_version')                      AS "server_version!",
                pg_database_size(current_database())                   AS "size_bytes!",
                (SELECT COUNT(*) FROM pg_stat_activity
                  WHERE datname = current_database())                  AS "connections!",
                current_setting('max_connections')::BIGINT             AS "max_connections!"
            "#,
        )
        .fetch_one(db)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "monitoring: database stats failed"))
        .ok()?;

        Some(DatabaseStatus {
            ping_us,
            server_version: row.server_version,
            size_bytes: row.size_bytes,
            connections: row.connections,
            max_connections: row.max_connections,
        })
    }

    async fn tables(db: &PgPool) -> Vec<TableSize> {
        sqlx::query_as!(
            TableSize,
            r#"
            SELECT
                relname::TEXT                     AS "name!",
                n_live_tup                        AS "rows!",
                pg_total_relation_size(relid)     AS "bytes!"
            FROM pg_stat_user_tables
            ORDER BY pg_total_relation_size(relid) DESC
            LIMIT 12
            "#,
        )
        .fetch_all(db)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "monitoring: table sizes failed"))
        .unwrap_or_default()
    }

    async fn migrations(db: &PgPool) -> Option<MigrationStatus> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE success)      AS "applied!",
                COUNT(*) FILTER (WHERE NOT success)  AS "failed!",
                MAX(version)                         AS latest_version
            FROM _sqlx_migrations
            "#,
        )
        .fetch_one(db)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "monitoring: migration status failed"))
        .ok()?;

        let latest = sqlx::query!(
            "SELECT description, installed_on FROM _sqlx_migrations ORDER BY version DESC LIMIT 1",
        )
        .fetch_optional(db)
        .await
        .ok()
        .flatten();

        Some(MigrationStatus {
            applied: row.applied,
            failed: row.failed,
            bundled: sqlx::migrate!("./migrations").iter().count(),
            latest_version: row.latest_version,
            latest_description: latest.as_ref().map(|l| l.description.clone()),
            latest_installed_at: latest.map(|l| l.installed_on),
        })
    }

    async fn scheduler(db: &PgPool) -> Option<SchedulerStatus> {
        sqlx::query!(
            r#"
            SELECT updated_at, held_until > NOW() AS "running!"
            FROM scheduler_locks
            WHERE key = 'daily'
            "#,
        )
        .fetch_optional(db)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "monitoring: scheduler status failed"))
        .ok()
        .flatten()
        .map(|row| SchedulerStatus {
            last_run_at: row.updated_at,
            running: row.running,
        })
    }

    async fn pulse(db: &PgPool) -> Option<Pulse> {
        sqlx::query_as!(
            Pulse,
            r#"
            SELECT
                (SELECT COUNT(*) FROM users
                  WHERE created_at >= NOW() - INTERVAL '24 hours')                  AS "new_users!",
                (SELECT COUNT(*) FROM users
                  WHERE last_seen_at >= NOW() - INTERVAL '24 hours')                AS "active_users!",
                (SELECT COUNT(*) FROM user_sign_ins
                  WHERE created_at >= NOW() - INTERVAL '24 hours')                  AS "sign_ins!",
                (SELECT COUNT(*) FROM users
                  WHERE NOT verified AND created_at >= NOW() - INTERVAL '24 hours') AS "unverified_sign_ups!",
                (SELECT COUNT(*) FROM users WHERE role = 'professional')           AS "pro_users!"
            "#,
        )
        .fetch_one(db)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "monitoring: pulse failed"))
        .ok()
    }
}

/// The commit this build came from: `GIT_HASH` at build time, else the one
/// Railway sets at runtime, else the crate version.
fn build_version() -> String {
    option_env!("GIT_HASH")
        .map(str::to_string)
        .or_else(|| std::env::var("RAILWAY_GIT_COMMIT_SHA").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// "3d 4h", "2h 15m", "42s".
pub fn humanize(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds().max(0);
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

/// "1.2 GB", "340 MB", "12 KB".
pub fn format_bytes(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_durations() {
        assert_eq!(humanize(chrono::Duration::seconds(42)), "42s");
        assert_eq!(humanize(chrono::Duration::minutes(135)), "2h 15m");
        assert_eq!(humanize(chrono::Duration::hours(76)), "3d 4h");
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(340 * 1024 * 1024), "340 MB");
    }
}
