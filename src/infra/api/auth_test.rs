//! Test-only authentication shim for end-to-end tests.
//!
//! Mounted by `src/infra/api/mod.rs` only when `ENABLE_TEST_AUTH=true`. Every
//! handler in this module also rejects any request whose `X-Test-Auth-Token`
//! header doesn't match the `TEST_AUTH_TOKEN` env var, so a misconfigured
//! production deploy still cannot grant arbitrary access.
//!
//! Routes:
//! - `POST /auth/test/sign-in-code` — issues a fresh emailed sign-in code for
//!   an address and returns it in plaintext, since tests can't read the inbox.

use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use axum_extra::extract::Form;

const ENABLE_ENV: &str = "ENABLE_TEST_AUTH";
const TOKEN_ENV: &str = "TEST_AUTH_TOKEN";
const TOKEN_HEADER: &str = "x-test-auth-token";

#[derive(Deserialize)]
struct TestSignInCodeForm {
    email: String,
}

pub fn routes() -> Router<SharedState> {
    Router::new().route("/auth/test/sign-in-code", post(test_sign_in_code))
}

pub fn is_enabled() -> bool {
    env::var(ENABLE_ENV)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn ensure_token(headers: &HeaderMap) -> Result<()> {
    let expected = env::var(TOKEN_ENV).map_err(|_| {
        AppError::Internal(format!("{TOKEN_ENV} must be set when test auth is enabled"))
    })?;
    let provided = headers
        .get(TOKEN_HEADER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    if provided != expected || expected.is_empty() {
        return Err(AppError::Forbidden("Invalid test auth token.".to_string()));
    }
    Ok(())
}

/// Issue a sign-in code the way `SendSignInCodeUseCase` would, minus the
/// email and the throttling, and hand back the plaintext. As the newest code
/// for the address it supersedes anything the UI requested.
async fn test_sign_in_code(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Form(form): Form<TestSignInCodeForm>,
) -> Result<impl IntoResponse> {
    ensure_token(&headers)?;

    let email = Email::parse(&form.email)?;
    let code = OneTimeCode::generate();
    OneTimePasscode::create(state.db.as_ref(), &email, &code).await?;
    Ok(code.as_str().to_string())
}
