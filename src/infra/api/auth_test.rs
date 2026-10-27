//! Test-only authentication shim for end-to-end tests.
//!
//! Mounted by `src/infra/api/mod.rs` only when `ENABLE_TEST_AUTH=true`. Every
//! handler in this module also rejects any request whose `X-Test-Auth-Token`
//! header doesn't match the `TEST_AUTH_TOKEN` env var, so a misconfigured
//! production deploy still cannot grant arbitrary access.
//!
//! Routes:
//! - `POST /auth/test/sign-in` — upserts a verified user with the given role,
//!   records a sign-in from the given IP, and drops their auth cookie.
//! - `POST /auth/test/sign-in-code` — issues a fresh emailed sign-in code for
//!   an address and returns it in plaintext, since tests can't read the inbox.

use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::sign_in::SignIn;
use crate::domain::user::{AuthProvider, CreateUser, Role, User};
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::infra::auth::jwt::{Jwt, UserClaims};
use crate::prelude::*;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum_extra::extract::cookie::{self, Cookie};
use axum_extra::extract::{CookieJar, Form};

const ENABLE_ENV: &str = "ENABLE_TEST_AUTH";
const TOKEN_ENV: &str = "TEST_AUTH_TOKEN";
const TOKEN_HEADER: &str = "x-test-auth-token";

#[derive(Deserialize)]
struct TestSignInForm {
    email: String,
    /// Optional — defaults to `free`. Always applied, so a test gets the role
    /// it asks for even if the account already exists (or was the first one
    /// created, which `User::create` makes an admin).
    role: Option<String>,
    full_name: Option<String>,
    /// Optional — defaults to `127.0.0.1`. The IP this sign-in is recorded
    /// from, so tests can give accounts distinct (or shared) sign-in IPs.
    ip: Option<String>,
}

#[derive(Deserialize)]
struct TestSignInCodeForm {
    email: String,
}

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/auth/test/sign-in", post(test_sign_in))
        .route("/auth/test/sign-in-code", post(test_sign_in_code))
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

async fn test_sign_in(
    State(state): State<SharedState>,
    headers: HeaderMap,
    cookies: CookieJar,
    Form(form): Form<TestSignInForm>,
) -> Result<impl IntoResponse> {
    ensure_token(&headers)?;

    let db = state.db.as_ref();
    let email = Email::parse(&form.email)?;
    let role = form.role.map(Role::from).unwrap_or_default();
    let ip = form.ip.unwrap_or_else(|| "127.0.0.1".to_string());
    let full_name = form.full_name.unwrap_or_else(|| "Test User".to_string());

    let user = match User::find_by_email(db, email.as_str()).await {
        Ok(user) => user,
        Err(_) => {
            User::create(
                db,
                &CreateUser {
                    country_id: None,
                    region_id: None,
                    city_id: None,
                    email: email.to_string(),
                    first_name: full_name.split(' ').next().unwrap_or_default().to_string(),
                    last_name: None,
                    full_name,
                    image_url: String::new(),
                    role: role.clone(),
                    verified: true,
                    auth_provider: AuthProvider::Google,
                    locked: false,
                    last_known_ip: ip.clone(),
                },
            )
            .await?
        }
    };
    User::set_role(db, user.id, &role).await?;
    let sign_in = CreateUser::from_existing(&user, ip);
    let user = User::update_sign_in_fields(db, user.id, &sign_in, user.locked).await?;
    SignIn::record(db, user.id, &user.last_known_ip, None, None, None).await?;

    let token = Jwt::generate(&UserClaims::from(user))?;
    // No `secure(true)`: Playwright drives a plain-HTTP localhost server.
    let cookie = Cookie::build(("auth_token", token))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .build();
    Ok((StatusCode::NO_CONTENT, cookies.add(cookie)))
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
