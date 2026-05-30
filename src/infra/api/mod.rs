use crate::SharedState;
use crate::domain::user::User;
use crate::infra::seo::PageMeta;
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
pub mod payments;
pub mod users;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .merge(core::routes())
        .merge(auth::routes())
        .merge(users::routes())
        .merge(countries::routes())
        .merge(announcements::routes())
        .merge(blogs::routes())
        .merge(payments::routes())
}

pub struct SharedContext {
    pub user: Option<User>,
    pub app_name: String,
    pub app_version: String,
    pub canonical_path: Option<String>,
    pub site_url: String,
    pub page_meta: PageMeta,
    pub default_title: String,
    pub default_description: String,
    pub default_image: String,
    pub default_robots: String,
    pub twitter_handle: String,
    pub web_app_name: String,
    pub web_app_short_name: String,
    pub theme_color: String,
    pub web_app_service_worker_enabled: bool,
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
            site_url: state.seo.site_url.clone(),
            page_meta: PageMeta::new(),
            default_title: state.seo.default_title.clone(),
            default_description: state.seo.default_description.clone(),
            default_image: state.seo.default_image.clone(),
            default_robots: state.seo.robots.clone(),
            twitter_handle: state.seo.twitter_handle.clone(),
            web_app_name: state.web_app.name.clone(),
            web_app_short_name: state.web_app.short_name.clone(),
            theme_color: state.web_app.theme_color.clone(),
            web_app_service_worker_enabled: state.web_app.service_worker_enabled,
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

    pub fn with_meta(self, page_meta: PageMeta) -> Self {
        Self { page_meta, ..self }
    }

    pub fn page_title(&self) -> &str {
        self.page_meta
            .title
            .as_deref()
            .unwrap_or(&self.default_title)
    }

    pub fn title_tag(&self) -> String {
        match &self.page_meta.title {
            Some(title) if title != &self.app_name => format!("{} — {}", title, self.app_name),
            _ => self.default_title.clone(),
        }
    }

    pub fn meta_description(&self) -> &str {
        self.page_meta
            .description
            .as_deref()
            .unwrap_or(&self.default_description)
    }

    pub fn meta_robots(&self) -> &str {
        self.page_meta
            .robots
            .as_deref()
            .unwrap_or(&self.default_robots)
    }

    pub fn canonical_url(&self) -> String {
        format!(
            "{}{}",
            self.site_url,
            self.canonical_path.as_deref().unwrap_or("/")
        )
    }

    pub fn image_url(&self) -> String {
        absolute_url(
            &self.site_url,
            self.page_meta
                .image_url
                .as_deref()
                .unwrap_or(&self.default_image),
        )
    }

    pub fn has_image(&self) -> bool {
        !self.image_url().is_empty()
    }

    pub fn has_twitter_handle(&self) -> bool {
        !self.twitter_handle.is_empty()
    }

    pub fn structured_data_json(&self) -> String {
        let mut data = serde_json::json!({
            "@context": "https://schema.org",
            "@type": if self.page_meta.kind == "article" { "Article" } else { "WebSite" },
            "name": self.page_title(),
            "description": self.meta_description(),
            "url": self.canonical_url(),
        });

        if self.page_meta.kind == "article" {
            data["headline"] = serde_json::json!(self.page_title());
            if let Some(published_at) = self.page_meta.published_at {
                data["datePublished"] = serde_json::json!(published_at.to_rfc3339());
            }
            if let Some(updated_at) = self.page_meta.updated_at {
                data["dateModified"] = serde_json::json!(updated_at.to_rfc3339());
            }
        }

        let image_url = self.image_url();
        if !image_url.is_empty() {
            data["image"] = serde_json::json!(image_url);
        }

        data.to_string()
    }
}

fn absolute_url(site_url: &str, value: &str) -> String {
    if value.is_empty() || value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else if value.starts_with('/') {
        format!("{}{}", site_url, value)
    } else {
        format!("{}/{}", site_url, value)
    }
}
