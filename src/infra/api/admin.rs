use crate::domain::admin_analytics::{AdminAnalytics, format_int};
use crate::domain::system_status::SystemStatus;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::monitoring::{self, LogEntry, LogLevel};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/admin/analytics", get(analytics))
        .route("/admin/monitoring", get(monitoring_page))
        .route("/admin/monitoring/status", get(monitoring_status))
        .route("/admin/monitoring/logs", get(monitoring_logs))
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/analytics.html")]
struct AnalyticsTemplate {
    shared: SharedContext,
    analytics: AdminAnalytics,
}

impl AnalyticsTemplate {
    fn int(&self, value: &i64) -> String {
        format_int(*value)
    }
}

async fn analytics(
    State(state): State<SharedState>,
    Admin(user): Admin,
) -> Result<AnalyticsTemplate> {
    let analytics = AdminAnalytics::load(state.db.as_ref()).await;
    Ok(AnalyticsTemplate {
        shared: SharedContext::new(&state)
            .with_user(Some(user))
            .with_canonical_path("/admin/analytics")
            .with_meta(
                PageMeta::new()
                    .title("Site Analytics")
                    .robots("noindex,nofollow"),
            ),
        analytics,
    })
}

/// How many log lines one view shows.
const LOG_PAGE: usize = 300;

/// Thousands separators for any whole number a monitoring template prints.
fn int(value: &dyn Display) -> String {
    value
        .to_string()
        .parse::<i64>()
        .map(format_int)
        .unwrap_or_else(|_| value.to_string())
}

#[derive(Deserialize, Default)]
struct LogQuery {
    #[serde(default)]
    level: String,
    #[serde(default)]
    q: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/monitoring.html")]
struct MonitoringTemplate {
    shared: SharedContext,
    status: SystemStatus,
    logs: Vec<LogEntry>,
    level: String,
    q: String,
}

impl MonitoringTemplate {
    fn int(&self, value: &dyn Display) -> String {
        int(value)
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/_monitoring_status.html")]
struct MonitoringStatusTemplate {
    status: SystemStatus,
}

impl MonitoringStatusTemplate {
    fn int(&self, value: &dyn Display) -> String {
        int(value)
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/_monitoring_logs.html")]
struct MonitoringLogsTemplate {
    logs: Vec<LogEntry>,
}

async fn monitoring_page(
    State(state): State<SharedState>,
    Admin(user): Admin,
    Query(query): Query<LogQuery>,
) -> Result<MonitoringTemplate> {
    let level = LogLevel::from_key(&query.level);
    Ok(MonitoringTemplate {
        status: SystemStatus::load(state.db.as_ref()).await,
        logs: monitoring::recent_logs(level, &query.q, LOG_PAGE),
        level: query.level,
        q: query.q,
        shared: SharedContext::new(&state)
            .with_user(Some(user))
            .with_canonical_path("/admin/monitoring")
            .with_meta(
                PageMeta::new()
                    .title("Monitoring")
                    .robots("noindex,nofollow"),
            ),
    })
}

async fn monitoring_status(
    State(state): State<SharedState>,
    Admin(_): Admin,
) -> Result<MonitoringStatusTemplate> {
    Ok(MonitoringStatusTemplate {
        status: SystemStatus::load(state.db.as_ref()).await,
    })
}

async fn monitoring_logs(Admin(_): Admin, Query(query): Query<LogQuery>) -> MonitoringLogsTemplate {
    MonitoringLogsTemplate {
        logs: monitoring::recent_logs(LogLevel::from_key(&query.level), &query.q, LOG_PAGE),
    }
}
