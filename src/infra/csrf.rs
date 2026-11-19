//! Cross-site request forgery protection for every state-changing request.
//!
//! `SameSite=Lax` keeps the session cookie off cross-site POSTs, but a
//! cross-site form can still *set* one: posting the attacker's email and code
//! to `/sign-in/code/verify` would sign the victim into the attacker's
//! account. So browser requests that change state must come from this site.
//! Browsers say where a request came from in `Sec-Fetch-Site` and `Origin`;
//! server-to-server callers (Stripe, the cron tick) send neither and pass.

use crate::prelude::*;
use axum::extract::Request;
use axum::http::{HeaderMap, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn same_origin_only(request: Request, next: Next) -> Response {
    let safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if !safe && !is_same_origin(request.headers()) {
        return AppError::Forbidden("Cross-site requests aren't allowed.".to_string())
            .into_response();
    }
    next.run(request).await
}

fn is_same_origin(headers: &HeaderMap) -> bool {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if header("sec-fetch-site") == Some("cross-site") {
        return false;
    }
    let Some(origin) = header(header::ORIGIN.as_str()) else {
        return true;
    };
    let Some(origin_host) = origin
        .split_once("://")
        .map(|(_, authority)| authority.to_ascii_lowercase())
    else {
        // `Origin: null` comes from sandboxed frames and file pages.
        return false;
    };
    // A proxy may rewrite `Host`, so the forwarded host and the configured
    // origin count as this site too.
    let app_origin = env::var("APP_ORIGIN").unwrap_or_default();
    let app_host = app_origin
        .split_once("://")
        .map(|(_, host)| host.trim_end_matches('/'));
    [header("host"), header("x-forwarded-host"), app_host]
        .into_iter()
        .flatten()
        .any(|host| host.eq_ignore_ascii_case(&origin_host))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn allows_same_origin_browser_requests() {
        let h = headers(&[("host", "app.test"), ("origin", "https://app.test")]);
        assert!(is_same_origin(&h));
        let h = headers(&[
            ("host", "internal:8080"),
            ("x-forwarded-host", "app.test"),
            ("origin", "https://app.test"),
        ]);
        assert!(is_same_origin(&h));
    }

    #[test]
    fn allows_requests_without_browser_headers() {
        assert!(is_same_origin(&headers(&[("host", "app.test")])));
    }

    #[test]
    fn refuses_cross_site_requests() {
        let h = headers(&[("host", "app.test"), ("origin", "https://evil.test")]);
        assert!(!is_same_origin(&h));
        let h = headers(&[("host", "app.test"), ("sec-fetch-site", "cross-site")]);
        assert!(!is_same_origin(&h));
        let h = headers(&[("host", "app.test"), ("origin", "null")]);
        assert!(!is_same_origin(&h));
    }
}
