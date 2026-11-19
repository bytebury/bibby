//! Google sign-in. Optional: without `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`
//! and `GOOGLE_CALLBACK_URL` the "Continue with Google" button is hidden and
//! `/auth/google` answers 404.

use crate::domain::user::{AuthProvider, CreateUser, Role};
use crate::infra::auth::oauth_state::OAuthState;
use crate::prelude::*;
use oauth2::basic::BasicClient;
use oauth2::*;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct GoogleUser {
    email: String,
    email_verified: bool,
    name: String,
    picture: String,
    given_name: Option<String>,
    family_name: Option<String>,
}

type Client = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet, EndpointSet>;

/// Whether Google sign-in is set up. Every page asks (for the button), so the
/// environment is read once.
pub fn is_configured() -> bool {
    static CONFIGURED: LazyLock<bool> = LazyLock::new(|| {
        let set = |name: &str| env::var(name).is_ok_and(|v| !v.trim().is_empty());
        set("GOOGLE_CLIENT_ID")
            && set("GOOGLE_CLIENT_SECRET")
            && env::var("GOOGLE_CALLBACK_URL").is_ok_and(|url| RedirectUrl::new(url).is_ok())
    });
    *CONFIGURED
}

pub fn ensure_configured() -> Result<()> {
    if is_configured() {
        Ok(())
    } else {
        Err(AppError::NotFound(
            "Google sign-in isn't set up here.".to_string(),
        ))
    }
}

/// Built once. Only used after `ensure_configured`, which checked the
/// callback URL parses.
fn client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        let url = |u: &str| u.to_string();
        BasicClient::new(ClientId::new(
            env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
        ))
        .set_client_secret(ClientSecret::new(
            env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
        ))
        .set_auth_uri(
            AuthUrl::new(url("https://accounts.google.com/o/oauth2/v2/auth")).expect("auth URL"),
        )
        .set_token_uri(
            TokenUrl::new(url("https://www.googleapis.com/oauth2/v3/token")).expect("token URL"),
        )
        .set_redirect_uri(
            RedirectUrl::new(env::var("GOOGLE_CALLBACK_URL").unwrap_or_default())
                .expect("GOOGLE_CALLBACK_URL"),
        )
        .set_revocation_url(
            RevocationUrl::new(url("https://oauth2.googleapis.com/revoke"))
                .expect("revocation URL"),
        )
    });
    &CLIENT
}

/// Shared by the token exchange and the profile fetch. Redirects are off, as
/// the oauth2 crate requires to avoid SSRF.
fn http() -> &'static reqwest::Client {
    static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("HTTP client")
    });
    &HTTP
}

/// The authorize URL, with a signed `state` naming the env this flow should
/// land back on. The single registered `GOOGLE_CALLBACK_URL` (prod) is the
/// redirect_uri for every env; the state is what lets the proxy decide where
/// to forward the code.
pub fn auth_url(target: &str, nonce: &str) -> Result<String> {
    let state = OAuthState::sign(target.to_string(), nonce.to_string())?;
    let (url, _) = client()
        .authorize_url(|| CsrfToken::new(state))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.profile".to_string(),
        ))
        .url();
    Ok(url.to_string())
}

/// Trades the callback's `code` for the Google profile it signs in as.
pub async fn exchange_code(code: &str) -> Result<CreateUser> {
    let token = client()
        .exchange_code(AuthorizationCode::new(code.to_string()))
        .request_async(http())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let google_user = http()
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token.access_token().secret())
        .send()
        .await?
        .error_for_status()?
        .json::<GoogleUser>()
        .await?;
    Ok(CreateUser::from(google_user))
}

impl From<GoogleUser> for CreateUser {
    fn from(google_user: GoogleUser) -> Self {
        Self {
            email: google_user.email,
            // Only an address Google has verified proves who owns it; an
            // unverified one can still verify itself with an emailed code.
            verified: google_user.email_verified,
            auth_provider: AuthProvider::Google,
            first_name: google_user.given_name.unwrap_or(google_user.name.clone()),
            last_name: google_user.family_name,
            full_name: google_user.name,
            image_url: google_user.picture,
            role: Role::Free,
            locked: false,
            country_id: None,
            region_id: None,
            city_id: None,
            last_known_ip: "".to_string(),
        }
    }
}
