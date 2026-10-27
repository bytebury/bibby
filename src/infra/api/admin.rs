use crate::domain::admin_analytics::{AdminAnalytics, format_int};
use crate::infra::api::SharedContext;
use crate::infra::api::extract::admin::Admin;
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::routing::get;

pub fn routes() -> Router<SharedState> {
    Router::new().route("/admin/analytics", get(analytics))
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
