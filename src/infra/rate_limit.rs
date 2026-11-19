//! Per-IP rate limiting for the routes that send email or check sign-in
//! codes. Per-address limits alone let one client spray codes at thousands of
//! addresses or create endless unverified accounts; this caps each IP too.
//!
//! Counts live in memory, so each server keeps its own and they start over
//! on restart, the same trade-off as `monitoring`.

use crate::infra::api::auth_test;
use crate::infra::api::extract::real_ip::RealIp;
use crate::prelude::*;
use axum::extract::{FromRequestParts, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration as StdDuration, Instant};

/// Requests one IP may make to the limited routes per window. Enough for
/// someone to sign up, mistype a few codes and ask for new ones.
const LIMIT: u32 = 20;
const WINDOW: StdDuration = StdDuration::from_secs(10 * 60);
/// Past this many tracked IPs, expired windows are swept on the next request.
const SWEEP_AT: usize = 10_000;

static AUTH: LazyLock<RateLimiter> = LazyLock::new(|| RateLimiter::new(LIMIT, WINDOW));

/// Fixed-window counter keyed by IP.
pub struct RateLimiter {
    limit: u32,
    window: StdDuration,
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window: StdDuration) -> Self {
        Self {
            limit,
            window,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Counts a request from `key` at `now`, returning whether it is allowed.
    pub fn allow(&self, key: &str, now: Instant) -> bool {
        let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        if windows.len() >= SWEEP_AT {
            windows.retain(|_, (started, _)| now.duration_since(*started) < self.window);
        }
        let (started, count) = windows.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(*started) >= self.window {
            *started = now;
            *count = 0;
        }
        *count += 1;
        *count <= self.limit
    }
}

/// Middleware for the sign-up and sign-in routes. It is off while the
/// test-auth shim is enabled, because the Playwright suite sends every
/// request from one IP.
pub async fn limit_auth(request: Request, next: Next) -> Response {
    if auth_test::is_enabled() {
        return next.run(request).await;
    }
    let (mut parts, body) = request.into_parts();
    let ip = match RealIp::from_request_parts(&mut parts, &std::sync::Arc::new(())).await {
        Ok(RealIp(ip)) => ip,
        Err(error) => return error.into_response(),
    };
    if !AUTH.allow(&ip, Instant::now()) {
        warn!(%ip, "rate limited a sign-in request");
        return AppError::TooManyRequests(
            "Too many attempts from your network. Try again in a few minutes.".to_string(),
        )
        .into_response();
    }
    next.run(Request::from_parts(parts, body)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit_per_window() {
        let limiter = RateLimiter::new(2, StdDuration::from_secs(60));
        let now = Instant::now();
        assert!(limiter.allow("1.1.1.1", now));
        assert!(limiter.allow("1.1.1.1", now));
        assert!(!limiter.allow("1.1.1.1", now));
        assert!(
            limiter.allow("2.2.2.2", now),
            "other IPs have their own count"
        );
    }

    #[test]
    fn a_new_window_starts_over() {
        let limiter = RateLimiter::new(1, StdDuration::from_secs(60));
        let now = Instant::now();
        assert!(limiter.allow("1.1.1.1", now));
        assert!(!limiter.allow("1.1.1.1", now));
        assert!(limiter.allow("1.1.1.1", now + StdDuration::from_secs(61)));
    }
}
