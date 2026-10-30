use crate::domain::one_time_passcode;
use crate::domain::user::{CreateUser, SignUp, SignUpForm, User};
use crate::domain::value_objects::email::Email;
use crate::infra::api::SharedContext;
use crate::infra::api::extract::maybe_current_user::MaybeCurrentUser;
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
use axum::routing::{delete, get, post};
use axum_extra::extract::cookie::{self, Cookie};
use axum_extra::extract::{CookieJar, Form};

const OAUTH_NONCE_COOKIE: &str = "oauth_nonce";

#[derive(Template, WebTemplate)]
#[template(path = "finish_sign_in.html")]
struct FinishSignInTemplate {
    shared: SharedContext,
}

#[derive(Template, WebTemplate)]
#[template(path = "sign_in.html")]
struct SignInTemplate {
    shared: SharedContext,
    /// `true` once a code has (maybe) been emailed — swaps the email field
    /// for the code field.
    code_sent: bool,
    /// The code being entered verifies a new email sign-up rather than
    /// signing an existing account in — only the copy changes.
    sign_up: bool,
    email: String,
    error_message: Option<String>,
    ttl_minutes: i64,
}

impl SignInTemplate {
    fn new(state: &SharedState) -> Self {
        Self {
            shared: SharedContext::new(state)
                .with_canonical_path("/sign-in")
                .with_meta(PageMeta::new().title("Sign in")),
            code_sent: false,
            sign_up: false,
            email: String::new(),
            error_message: None,
            ttl_minutes: one_time_passcode::TTL_MINUTES,
        }
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "sign_up.html")]
struct SignUpTemplate {
    shared: SharedContext,
    /// `true` once the account exists and a code has (maybe) been emailed —
    /// swaps the sign-up form for the code field.
    code_sent: bool,
    first_name: String,
    last_name: String,
    email: String,
    error_message: Option<String>,
    ttl_minutes: i64,
}

impl SignUpTemplate {
    fn new(state: &SharedState) -> Self {
        Self {
            shared: SharedContext::new(state)
                .with_canonical_path("/sign-up")
                .with_meta(PageMeta::new().title("Sign up")),
            code_sent: false,
            first_name: String::new(),
            last_name: String::new(),
            email: String::new(),
            error_message: None,
            ttl_minutes: one_time_passcode::TTL_MINUTES,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SendCodeForm {
    email: String,
    #[serde(default)]
    sign_up: bool,
}

#[derive(Debug, Deserialize)]
struct VerifyCodeForm {
    email: String,
    code: String,
    #[serde(default)]
    sign_up: bool,
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
        .route("/sign-in", get(sign_in_page))
        .route("/sign-in/code", post(send_sign_in_code))
        .route("/sign-in/code/verify", post(verify_sign_in_code))
        .route("/sign-up", get(sign_up_page).post(sign_up_with_email))
        .route("/auth/google", get(sign_in_with_google))
        .route("/auth/google/callback", get(sign_in_with_google_callback))
        .route("/auth", delete(sign_out))
}

async fn sign_in_page(
    State(state): State<SharedState>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    headers: HeaderMap,
) -> impl IntoResponse {
    if user.is_some() {
        return redirect!("/", &headers);
    }
    SignInTemplate::new(&state).into_response()
}

async fn sign_up_page(
    State(state): State<SharedState>,
    MaybeCurrentUser(user): MaybeCurrentUser,
    headers: HeaderMap,
) -> impl IntoResponse {
    if user.is_some() {
        return redirect!("/", &headers);
    }
    SignUpTemplate::new(&state).into_response()
}

/// Creates an unverified account and emails it a code. Entering the code on
/// `/sign-in/code/verify` verifies the account and signs it in. As with
/// `send_sign_in_code`, an address that already has an account gets the same
/// response, so this can't be used to probe for accounts.
///
/// A visitor from a locked country is sent home before anything is validated,
/// created, or emailed, and with no message. The check keys on the IP alone,
/// so it can't be used to probe for accounts either.
async fn sign_up_with_email(
    State(state): State<SharedState>,
    RealIp(ip_addr): RealIp,
    headers: HeaderMap,
    Form(form): Form<SignUpForm>,
) -> Result<impl IntoResponse> {
    if state
        .user_use_cases
        .register
        .country_locked(&ip_addr)
        .await?
    {
        return Ok(redirect!("/", &headers));
    }

    let page = SignUpTemplate {
        first_name: form.first_name.clone(),
        last_name: form.last_name.clone(),
        email: form.email.clone(),
        ..SignUpTemplate::new(&state)
    };

    let sign_up = match SignUp::try_from(form) {
        Ok(sign_up) => sign_up,
        Err(AppError::BadRequest(message)) => {
            return Ok(SignUpTemplate {
                error_message: Some(message),
                ..page
            }
            .into_response());
        }
        Err(error) => return Err(error),
    };

    state
        .user_use_cases
        .sign_up_with_email
        .execute(&sign_up, &ip_addr)
        .await?;

    Ok(SignUpTemplate {
        code_sent: true,
        email: sign_up.email.to_string(),
        ..page
    }
    .into_response())
}

/// Emails a sign-in code. The response is the same whether or not the address
/// has an account, so this can't be used to probe for accounts.
async fn send_sign_in_code(
    State(state): State<SharedState>,
    Form(form): Form<SendCodeForm>,
) -> Result<impl IntoResponse> {
    let page = SignInTemplate {
        email: form.email.trim().to_string(),
        sign_up: form.sign_up,
        ..SignInTemplate::new(&state)
    };

    let email = match Email::parse(&form.email) {
        Ok(email) => email,
        Err(AppError::BadRequest(message)) => {
            return Ok(SignInTemplate {
                error_message: Some(message),
                ..page
            });
        }
        Err(error) => return Err(error),
    };

    state
        .user_use_cases
        .send_sign_in_code
        .execute(&email)
        .await?;

    Ok(SignInTemplate {
        code_sent: true,
        email: email.to_string(),
        ..page
    })
}

async fn verify_sign_in_code(
    State(state): State<SharedState>,
    RealIp(ip_addr): RealIp,
    headers: HeaderMap,
    cookies: CookieJar,
    Form(form): Form<VerifyCodeForm>,
) -> Result<impl IntoResponse> {
    let verified = match Email::parse(&form.email) {
        Ok(email) => {
            state
                .user_use_cases
                .verify_sign_in_code
                .execute(&email, &form.code)
                .await
        }
        Err(error) => Err(error),
    };
    let user = match verified {
        Ok(user) => user,
        Err(AppError::BadRequest(message)) => {
            return Ok(SignInTemplate {
                code_sent: true,
                sign_up: form.sign_up,
                email: form.email.trim().to_string(),
                error_message: Some(message),
                ..SignInTemplate::new(&state)
            }
            .into_response());
        }
        Err(error) => return Err(error),
    };

    // Same path a Google sign-in takes, so the location and lock signals are
    // refreshed.
    let mut sign_in = CreateUser::from_existing(&user, ip_addr);
    let user = state.user_use_cases.register.execute(&mut sign_in).await?;

    if user.locked || !user.verified {
        return Ok(redirect!("/", &headers));
    }

    Ok((cookies.add(auth_cookie(&user)?), redirect!("/", &headers)).into_response())
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

    // A first-time sign-in from a locked country never creates an account.
    // Existing accounts fall through to `execute`, which locks them instead.
    if state
        .user_use_cases
        .register
        .blocks_new_account(&create_user.email, &create_user.last_known_ip)
        .await?
    {
        let cookies = cookies.remove(Cookie::build(OAUTH_NONCE_COOKIE).path("/"));
        return Ok((cookies, redirect!("/", &headers)).into_response());
    }

    let user = state
        .user_use_cases
        .register
        .execute(&mut create_user)
        .await?;

    // Locked accounts (banned country or an admin lock) never get a session —
    // send them back to the homepage. Neither does an address Google hasn't
    // verified; an emailed code can verify it instead.
    if user.locked || !user.verified {
        let cookies = cookies.remove(Cookie::build(OAUTH_NONCE_COOKIE).path("/"));
        let target = if user.locked { "/" } else { "/sign-in" };
        return Ok((cookies, redirect!(target, &headers)).into_response());
    }

    let cookies = cookies
        .add(auth_cookie(&user)?)
        .remove(Cookie::build(OAUTH_NONCE_COOKIE).path("/"));

    Ok((cookies, redirect!("/", &headers)).into_response())
}

/// Mint the session cookie for `user`.
pub fn auth_cookie(user: &User) -> Result<Cookie<'static>> {
    let token = Jwt::generate(&UserClaims::from(user.clone()))?;
    Ok(Cookie::build(("auth_token", token))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .secure(
            env::var("APP_ORIGIN")
                .unwrap_or_default()
                .starts_with("https://"),
        )
        .build())
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
