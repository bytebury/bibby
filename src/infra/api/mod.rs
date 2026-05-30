use crate::SharedState;
use crate::domain::user::User;
use axum::Router;
use chrono::Datelike;

/// Redirects the user appropriately based on the request origin.
///
/// Standard browser requests receive a typical 303 Redirect.
/// HTMX requests receive a 200 OK with the `HX-Redirect` header,
/// preventing HTMX from swapping the redirect target into the
/// current element and forcing a full-page navigation instead.
#[macro_export]
macro_rules! redirect {
    ($url:expr, $headers:expr) => {{
        use axum::response::IntoResponse;
        if $headers.contains_key("HX-Request") {
            (axum::http::StatusCode::OK, [("HX-Redirect", $url)]).into_response()
        } else {
            axum::response::Redirect::to($url).into_response()
        }
    }};
}

/// Refreshes the browser.
#[macro_export]
macro_rules! refresh {
    ($headers:expr) => {{
        use axum::response::IntoResponse;
        if $headers.contains_key("HX-Request") {
            (axum::http::StatusCode::OK, [("HX-Refresh", "true")]).into_response()
        } else {
            (axum::http::StatusCode::OK, [("Refresh", "0")]).into_response()
        }
    }};
}

pub mod announcements;
pub mod auth;
pub mod blogs;
pub mod core;
pub mod countries;
pub mod extract;
pub mod users;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .merge(core::routes())
        .merge(auth::routes())
        .merge(users::routes())
        .merge(countries::routes())
        .merge(announcements::routes())
        .merge(blogs::routes())
}

pub struct SharedContext {
    pub user: Option<User>,
    pub app_name: String,
    pub app_version: String,
    pub canonical_path: Option<String>,
    /// Current UTC year — stamped on each request and used by the footer
    /// copyright line so it rolls over without a code change.
    pub current_year: i32,
}

impl SharedContext {
    pub fn new(state: &SharedState) -> Self {
        Self {
            app_name: state.app_name.clone(),
            app_version: state.app_version.clone(),
            user: None,
            canonical_path: None,
            current_year: chrono::Utc::now().year(),
        }
    }

    pub fn with_user(self, user: Option<User>) -> Self {
        Self { user, ..self }
    }

    pub fn with_canonical_path(self, path: impl Into<String>) -> Self {
        Self {
            canonical_path: Some(path.into()),
            ..self
        }
    }
}
