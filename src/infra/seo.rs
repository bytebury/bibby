use crate::domain::blog::Blog;
use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct SeoConfig {
    pub site_url: String,
    pub default_title: String,
    pub default_description: String,
    pub default_image: String,
    pub twitter_handle: String,
    pub robots: String,
}

impl SeoConfig {
    pub fn from_env(app_name: &str) -> Self {
        let default_description = format!("{} is a full-stack Rust web application.", app_name);
        let local_origin = local_origin_from_port();
        Self {
            site_url: env::var("PUBLIC_SITE_URL")
                .or_else(|_| env::var("APP_ORIGIN"))
                .unwrap_or(local_origin)
                .trim_end_matches('/')
                .to_string(),
            default_title: env::var("SEO_DEFAULT_TITLE").unwrap_or_else(|_| app_name.to_string()),
            default_description: env::var("SEO_DEFAULT_DESCRIPTION").unwrap_or(default_description),
            default_image: env::var("SEO_DEFAULT_IMAGE").unwrap_or_default(),
            twitter_handle: env::var("SEO_TWITTER_HANDLE").unwrap_or_default(),
            robots: env::var("SEO_ROBOTS").unwrap_or_else(|_| "index,follow".to_string()),
        }
    }
}

fn local_origin_from_port() -> String {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://localhost:{port}")
}

#[derive(Debug, Clone)]
pub struct WebAppConfig {
    pub name: String,
    pub short_name: String,
    pub theme_color: String,
    pub background_color: String,
    pub display: String,
    pub service_worker_enabled: bool,
}

impl WebAppConfig {
    pub fn from_env(app_name: &str) -> Self {
        Self {
            name: env::var("WEB_APP_NAME").unwrap_or_else(|_| app_name.to_string()),
            short_name: env::var("WEB_APP_SHORT_NAME").unwrap_or_else(|_| app_name.to_string()),
            theme_color: env::var("WEB_APP_THEME_COLOR").unwrap_or_else(|_| "#111827".to_string()),
            background_color: env::var("WEB_APP_BACKGROUND_COLOR")
                .unwrap_or_else(|_| "#f9fafb".to_string()),
            display: env::var("WEB_APP_DISPLAY").unwrap_or_else(|_| "standalone".to_string()),
            service_worker_enabled: env::var("WEB_APP_SERVICE_WORKER")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub robots: Option<String>,
    pub kind: String,
    pub published_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Default for PageMeta {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            image_url: None,
            robots: None,
            kind: "website".to_string(),
            published_at: None,
            updated_at: None,
        }
    }
}

impl PageMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn image_url(mut self, image_url: impl Into<String>) -> Self {
        self.image_url = Some(image_url.into());
        self
    }

    pub fn robots(mut self, robots: impl Into<String>) -> Self {
        self.robots = Some(robots.into());
        self
    }

    pub fn article(blog: &Blog) -> Self {
        let mut meta = Self::new()
            .title(&blog.title)
            .description(blog.excerpt())
            .kind("article");
        if !blog.image_url.is_empty() {
            meta = meta.image_url(&blog.image_url);
        }
        meta.published_at = Some(blog.created_at);
        meta.updated_at = Some(blog.updated_at);
        meta
    }

    fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }
}
