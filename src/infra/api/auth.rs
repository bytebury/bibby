use crate::infra::api::SharedContext;
use crate::infra::api::extract::real_ip::RealIp;
use crate::infra::auth::OAuthProvider;
use crate::infra::auth::google::GoogleOAuth;
use crate::infra::auth::jwt::{Jwt, UserClaims};
use crate::infra::auth::oauth_state::{OAuthState, is_allowed_target};
use crate::infra::seo::PageMeta;
use crate::prelude::*;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{self, Cookie};

const OAUTH_NONCE_COOKIE: &str = "oauth_nonce";

#[derive(Template, WebTemplate)]
#[template(path = "finish_sign_in.html")]
struct FinishSignInTemplate {
    shared: SharedContext,
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct SignOutQuery {
    redirect: Option<String>,
}

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/auth/google", get(sign_in_with_google))
        .route("/auth/google/callback", get(sign_in_with_google_callback))
        .route("/auth", delete(sign_out))
}

async fn sign_in_with_google(headers: HeaderMap, cookies: CookieJar) -> Result<impl IntoResponse> {
    let origin =
        env::var("APP_ORIGIN").map_err(|_| AppError::Internal("APP_ORIGIN not set".to_string()))?;
    let nonce = OAuthState::new_nonce();
    let url = GoogleOAuth::default().auth_url_for_target(&origin, &nonce)?;

    // Pin this flow to this browser. The callback compares this cookie to
    // `state.nonce`; if they don't match the user finished OAuth in a
    // different browser context (common on mobile) and we route them through
    // an interstitial instead of silently dropping the sign-in.
    let nonce_cookie = Cookie::build((OAUTH_NONCE_COOKIE, nonce))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .secure(
            env::var("APP_ORIGIN")
                .unwrap_or_default()
                .starts_with("https://"),
        );
    let cookies = cookies.add(nonce_cookie);

    Ok((cookies, redirect!(url.as_str(), &headers)))
}

async fn sign_in_with_google_callback(
    State(state): State<SharedState>,
    Query(params): Query<AuthRequest>,
    RealIp(ip_addr): RealIp,
    headers: HeaderMap,
    cookies: CookieJar,
) -> Result<impl IntoResponse> {
    let oauth_state = OAuthState::verify(&params.state)
        .map_err(|_| AppError::BadRequest("Invalid or expired OAuth state.".to_string()))?;

    let our_origin = env::var("APP_ORIGIN").unwrap_or_default();

    // If state points elsewhere, we're the registered proxy — forward the
    // code+state onward without consuming them. Google's token endpoint only
    // checks that `redirect_uri` matches between authorize and exchange (a
    // string comparison, not a live callback), so the destination env can
    // complete the exchange using the same prod URL we used here.
    if oauth_state.target != our_origin {
        if !is_allowed_target(&oauth_state.target) {
            return Err(AppError::Forbidden("OAuth target not allowed.".to_string()));
        }
        let relay = format!(
            "{}/auth/google/callback?code={}&state={}",
            oauth_state.target.trim_end_matches('/'),
            params.code,
            params.state,
        );
        return Ok(redirect!(relay.as_str(), &headers).into_response());
    }

    // The browser that started this flow set `oauth_nonce` to `state.nonce`.
    // If it's missing/different, the callback landed in a different browser
    // (Custom Tabs handoff, in-app browser → system browser, etc.) — render
    // an interstitial that restarts the flow in *this* browser so the auth
    // cookie ends up where the user actually is.
    let nonce_matches = cookies
        .get(OAUTH_NONCE_COOKIE)
        .map(|c| c.value() == oauth_state.nonce)
        .unwrap_or(false);
    if !nonce_matches {
        return Ok(FinishSignInTemplate {
            shared: SharedContext::new(&state)
                .with_canonical_path("/auth/google")
                .with_meta(
                    PageMeta::new()
                        .title("Finish signing in")
                        .robots("noindex,nofollow"),
                ),
        }
        .into_response());
    }

    let mut create_user = GoogleOAuth::default()
        .exchange_code_for_user(&params.code)
        .await?;
    create_user.last_known_ip = ip_addr;

    let user = state
        .user_use_cases
        .register
        .execute(&mut create_user)
        .await?;

    let token = Jwt::generate(&UserClaims::from(user))?;
    let auth_cookie = Cookie::build(("auth_token", token))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .secure(
            env::var("APP_ORIGIN")
                .unwrap_or_default()
                .starts_with("https://"),
        );
    let cookies = cookies
        .add(auth_cookie)
        .remove(Cookie::build(OAUTH_NONCE_COOKIE).path("/"));

    Ok((cookies, redirect!("/", &headers)).into_response())
}

async fn sign_out(
    Query(params): Query<SignOutQuery>,
    headers: HeaderMap,
    cookies: CookieJar,
) -> impl IntoResponse {
    let cookies = cookies.remove(Cookie::build("auth_token").path("/"));
    let target = params
        .redirect
        .filter(|r| r.starts_with('/') && !r.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());
    (cookies, redirect!(&target, &headers))
}
