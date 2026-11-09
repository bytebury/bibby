use crate::infra::scheduler;
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;

pub fn routes() -> Router<SharedState> {
    Router::new().route("/_internal/cron/tick", post(tick))
}

/// Hit by a cron service on schedule (daily is enough) to drive the daily
/// jobs even when the web service has been asleep. Protected by a shared
/// secret in `X-Cron-Secret`, compared in constant time so it can't be
/// brute-forced via response timing. The tick itself is coordinated across
/// replicas by the Postgres-backed lease inside `run_tick`.
///
/// Returns immediately and runs the tick on a background task so the cron
/// client doesn't time out while the web service is cold-starting.
async fn tick(State(state): State<SharedState>, headers: HeaderMap) -> StatusCode {
    let Ok(expected) = env::var("CRON_SECRET") else {
        error!("CRON_SECRET not set; refusing cron tick");
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let provided = headers
        .get("x-cron-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if expected.is_empty() || !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return StatusCode::UNAUTHORIZED;
    }
    tokio::spawn(async move {
        scheduler::run_tick(&state).await;
    });
    StatusCode::ACCEPTED
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn compares_secrets() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret!"));
    }
}
