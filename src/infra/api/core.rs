use crate::domain::blog::Blog;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::maybe_current_user::MaybeCurrentUser;
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::get;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/", get(homepage))
        .route("/privacy", get(privacy))
        .route("/terms", get(terms))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/site.webmanifest", get(site_webmanifest))
        .route("/service-worker.js", get(service_worker_js))
}

#[derive(Template, WebTemplate)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "privacy.html")]
struct PrivacyTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "terms.html")]
struct TermsTemplate {
    shared: SharedContext,
}

async fn homepage(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> HomepageTemplate {
    HomepageTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/")
            .with_meta(PageMeta::new().description(
                "A full-stack Rust template at the heart of every Bytebury application.",
            )),
    }
}

async fn privacy(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> PrivacyTemplate {
    PrivacyTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/privacy")
            .with_meta(PageMeta::new().title("Privacy Policy")),
    }
}

async fn terms(
    MaybeCurrentUser(user): MaybeCurrentUser,
    State(state): State<SharedState>,
) -> TermsTemplate {
    TermsTemplate {
        shared: SharedContext::new(&state)
            .with_user(user)
            .with_canonical_path("/terms")
            .with_meta(PageMeta::new().title("Terms of Service")),
    }
}

async fn robots_txt(State(state): State<SharedState>) -> impl IntoResponse {
    let body = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        state.seo.site_url
    );
    ([(CONTENT_TYPE, "text/plain; charset=utf-8")], body)
}

async fn sitemap_xml(State(state): State<SharedState>) -> Result<impl IntoResponse> {
    let mut urls = vec![
        sitemap_url(&state.seo.site_url, "/", None),
        sitemap_url(&state.seo.site_url, "/blogs", None),
        sitemap_url(&state.seo.site_url, "/privacy", None),
        sitemap_url(&state.seo.site_url, "/terms", None),
    ];

    for blog in Blog::find_all_for_sitemap(state.db.as_ref()).await? {
        urls.push(sitemap_url(
            &state.seo.site_url,
            &format!("/blogs/{}/{}", blog.id, blog.slug()),
            Some(blog.updated_at),
        ));
    }

    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}
</urlset>
"#,
        urls.join("")
    );
    Ok(([(CONTENT_TYPE, "application/xml; charset=utf-8")], body))
}

async fn site_webmanifest(State(state): State<SharedState>) -> impl IntoResponse {
    let manifest = serde_json::json!({
        "name": state.web_app.name,
        "short_name": state.web_app.short_name,
        "start_url": "/",
        "scope": "/",
        "display": state.web_app.display,
        "background_color": state.web_app.background_color,
        "theme_color": state.web_app.theme_color,
        "icons": [
            {
                "src": "/assets/images/app-icon.svg",
                "sizes": "any",
                "type": "image/svg+xml",
                "purpose": "any maskable"
            }
        ]
    });
    (
        [(CONTENT_TYPE, "application/manifest+json; charset=utf-8")],
        manifest.to_string(),
    )
}

async fn service_worker_js() -> impl IntoResponse {
    let body = r#"self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));
"#;
    ([(CONTENT_TYPE, "text/javascript; charset=utf-8")], body)
}

fn sitemap_url(site_url: &str, path: &str, updated_at: Option<DateTime<Utc>>) -> String {
    let lastmod = updated_at
        .map(|updated_at| format!("    <lastmod>{}</lastmod>\n", updated_at.to_rfc3339()))
        .unwrap_or_default();
    format!(
        "  <url>\n    <loc>{}{}</loc>\n{}</url>\n",
        xml_escape(site_url),
        xml_escape(path),
        lastmod
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
