use crate::SharedState;
use crate::domain::feature_toggle::Feature;
use crate::domain::user::User;
use crate::infra::seo::PageMeta;
use crate::infra::site::SiteSnapshot;
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

/// A public page that needs nothing but the shared context: defines the
/// handler `$handler` rendering `templates/$template` at `$path`.
///
/// ```ignore
/// static_page!(about, "about.html", "/about", PageMeta::new().title("About"));
/// // then: .route("/about", get(about))
/// ```
#[macro_export]
macro_rules! static_page {
    ($handler:ident, $template:literal, $path:literal, $meta:expr) => {
        async fn $handler(
            $crate::infra::api::extract::maybe_current_user::MaybeCurrentUser(user): $crate::infra::api::extract::maybe_current_user::MaybeCurrentUser,
            axum::extract::State(state): axum::extract::State<$crate::SharedState>,
        ) -> impl axum::response::IntoResponse {
            #[derive(askama::Template, askama_web::WebTemplate)]
            #[template(path = $template)]
            struct Page {
                shared: $crate::infra::api::SharedContext,
            }
            Page {
                shared: $crate::infra::api::SharedContext::new(&state).await
                    .with_user(user)
                    .with_canonical_path($path)
                    .with_meta($meta),
            }
        }
    };
}

pub mod admin;
pub mod announcements;
pub mod auth;
pub mod auth_test;
pub mod blogs;
pub mod core;
pub mod countries;
pub mod cron;
pub mod extract;
pub mod feature_toggles;
pub mod payments;
pub mod settings;
pub mod users;

pub fn routes() -> Router<SharedState> {
    let router = Router::new()
        .merge(core::routes())
        .merge(admin::routes())
        .merge(feature_toggles::routes())
        .merge(cron::routes())
        .merge(auth::routes())
        .merge(users::routes())
        .merge(settings::routes())
        .merge(countries::routes())
        .merge(announcements::routes())
        .merge(blogs::routes())
        .merge(payments::routes());

    if auth_test::is_enabled() {
        tracing::warn!("Test auth shim is enabled. Never enable this in production.");
        return router.merge(auth_test::routes());
    }
    router
}

pub struct SharedContext {
    /// App-wide config (name, SEO defaults, web-app manifest, assets). An
    /// `Arc`, so building a context per page copies nothing.
    pub state: SharedState,
    pub user: Option<User>,
    pub canonical_path: Option<String>,
    pub page_meta: PageMeta,
    /// The banner and feature toggles, read for this page.
    pub site: SiteSnapshot,
    /// Current UTC year — stamped on each request and used by the footer
    /// copyright line so it rolls over without a code change.
    pub current_year: i32,
}

impl SharedContext {
    pub async fn new(state: &SharedState) -> Self {
        Self {
            state: state.clone(),
            user: None,
            canonical_path: None,
            page_meta: PageMeta::new(),
            site: SiteSnapshot::load(state.db.as_ref()).await,
            current_year: chrono::Utc::now().year(),
        }
    }

    pub fn app_name(&self) -> &str {
        &self.state.app_name
    }

    /// Whether Stripe is configured, so billing links can be hidden.
    pub fn payments_enabled(&self) -> bool {
        self.state.payments_use_cases.is_some()
    }

    /// The Google tag to load on every page, when that toggle is on.
    pub fn google_analytics_tag(&self) -> Option<&str> {
        self.site.toggles.value(Feature::GoogleAnalytics)
    }

    /// Whether Google sign-in is configured.
    pub fn google_enabled(&self) -> bool {
        crate::infra::auth::google::is_configured()
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

    /// URL for a file under `/assets/` with its content hash appended, so
    /// browsers cache it until it changes. See `infra/assets.rs`.
    pub fn asset(&self, path: &str) -> String {
        self.state.assets.url(path)
    }

    pub fn page_title(&self) -> &str {
        self.page_meta
            .title
            .as_deref()
            .unwrap_or(&self.state.seo.default_title)
    }

    pub fn title_tag(&self) -> String {
        match &self.page_meta.title {
            Some(title) if title != &self.state.app_name => {
                format!("{} — {}", title, self.state.app_name)
            }
            _ => self.state.seo.default_title.clone(),
        }
    }

    pub fn meta_description(&self) -> &str {
        self.page_meta
            .description
            .as_deref()
            .unwrap_or(&self.state.seo.default_description)
    }

    pub fn meta_robots(&self) -> &str {
        self.page_meta
            .robots
            .as_deref()
            .unwrap_or(&self.state.seo.robots)
    }

    pub fn canonical_url(&self) -> String {
        format!(
            "{}{}",
            self.state.seo.site_url,
            self.canonical_path.as_deref().unwrap_or("/")
        )
    }

    pub fn image_url(&self) -> String {
        absolute_url(
            &self.state.seo.site_url,
            self.page_meta
                .image_url
                .as_deref()
                .unwrap_or(&self.state.seo.default_image),
        )
    }

    pub fn has_image(&self) -> bool {
        !self.image_url().is_empty()
    }

    pub fn has_twitter_handle(&self) -> bool {
        !self.state.seo.twitter_handle.is_empty()
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

        script_json(&data)
    }
}

/// Serializes `value` for embedding inside a `<script>` element. A user's
/// name in the JSON could otherwise close the element with `</script>`, so
/// `<`, `>` and `&` are written as JSON unicode escapes, which parse to the
/// same string.
pub fn script_json(value: &serde_json::Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
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

#[cfg(test)]
mod tests {
    use super::script_json;

    #[test]
    fn json_in_a_script_tag_cannot_close_it() {
        let name = "</script><script>alert(1)</script> & <!--";
        let json = script_json(&serde_json::json!({ "name": name }));
        assert!(!json.contains('<') && !json.contains('>') && !json.contains('&'));
        // The escapes are JSON, so the value reads back unchanged.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], name);
    }
}
