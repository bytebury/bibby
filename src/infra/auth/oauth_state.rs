use crate::infra::auth::jwt::Jwt;
use crate::prelude::*;
use url::Url;

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
    let Ok(allowed) = env::var("OAUTH_ALLOWED_TARGETS") else {
        return false;
    };
    target_matches(target, &allowed)
}

fn target_matches(target: &str, allowed: &str) -> bool {
    let Some(host) = origin_host(target) else {
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

/// The `host[:port]` of `target` when it is a bare https origin. Anything
/// carrying credentials, a path, a query or a fragment is refused, so
/// `https://evil.com#.up.railway.app` can't pass for a Railway host.
fn origin_host(target: &str) -> Option<String> {
    let url = Url::parse(target).ok()?;
    let bare = url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none();
    if !bare {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

#[cfg(test)]
mod tests {
    use super::target_matches;

    const RULES: &str = "app.example.com, *.up.railway.app";

    #[test]
    fn allows_listed_origins_and_wildcard_hosts() {
        assert!(target_matches("https://app.example.com", RULES));
        assert!(target_matches("https://bibby-pr-1.up.railway.app/", RULES));
    }

    #[test]
    fn refuses_anything_that_is_not_a_bare_https_origin() {
        assert!(!target_matches("https://evil.com#.up.railway.app", RULES));
        assert!(!target_matches("https://evil.com/.up.railway.app", RULES));
        assert!(!target_matches("https://evil.com?.up.railway.app", RULES));
        assert!(!target_matches("https://app.example.com@evil.com", RULES));
        assert!(!target_matches("http://app.example.com", RULES));
        assert!(!target_matches("https://evil.com", RULES));
    }
}
