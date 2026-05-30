use crate::infra::auth::jwt::Jwt;
use crate::prelude::*;

/// Signed payload smuggled through the OAuth `state=` parameter so the prod
/// host can act as a proxy for preview-env redirect URIs. HMAC-signed with
/// `JWT_SECRET` — clients can't forge a redirect to an attacker's host.
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub target: String,
    pub nonce: String,
    pub exp: i64,
}

impl OAuthState {
    pub fn new_nonce() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn sign(target: String, nonce: String) -> Result<String> {
        let exp = (Utc::now() + Duration::minutes(15)).timestamp();
        Jwt::generate(&OAuthState { target, nonce, exp })
    }

    pub fn verify(token: &str) -> Result<OAuthState> {
        Jwt::verify::<OAuthState>(token).map(|d| d.claims)
    }
}

/// Whether `target` is an origin this env is willing to relay auth codes to.
/// Configured via `OAUTH_ALLOWED_TARGETS` — comma-separated list of origins
/// or hosts. A rule may contain a single `*` anywhere (e.g. `*.up.railway.app`).
pub fn is_allowed_target(target: &str) -> bool {
    let Some(host) = target
        .strip_prefix("https://")
        .and_then(|s| s.split('/').next())
        .map(str::to_ascii_lowercase)
    else {
        return false;
    };
    let Ok(allowed) = env::var("OAUTH_ALLOWED_TARGETS") else {
        return false;
    };
    allowed
        .split(',')
        .map(|r| {
            r.trim()
                .trim_start_matches("https://")
                .trim_end_matches('/')
                .to_ascii_lowercase()
        })
        .filter(|r| !r.is_empty())
        .any(|rule| match rule.split_once('*') {
            None => host == rule,
            Some((left, right)) => {
                host.len() >= left.len() + right.len()
                    && host.starts_with(left)
                    && host.ends_with(right)
            }
        })
}
