use crate::prelude::*;
use jsonwebtoken::*;

pub mod user_claims;

pub use user_claims::{SESSION_DAYS, UserClaims};

pub struct Jwt {}

impl Jwt {
    pub fn generate<T: Serialize>(claims: &T) -> Result<String> {
        let encoding_key = EncodingKey::from_secret(&secret());
        encode(&Header::new(Algorithm::HS256), claims, &encoding_key)
            .map_err(|e| AppError::Internal(format!("Unable to generate JWT: {}", e)))
    }

    pub fn verify<T: DeserializeOwned>(token: &str) -> Result<TokenData<T>> {
        let decoding_key = DecodingKey::from_secret(&secret());
        decode::<T>(token, &decoding_key, &Validation::new(Algorithm::HS256))
            .map_err(|e| AppError::Internal(format!("Unable to authenticate user: {}", e)))
    }
}

/// The shortest `JWT_SECRET` accepted. It signs sessions and OAuth state and
/// keys the sign-in code hashes, so a guessable one hands out sessions.
pub const MIN_SECRET_LEN: usize = 32;

impl Jwt {
    /// Stops startup when `JWT_SECRET` is missing, too short, or the
    /// placeholder older generated `.env` files carried.
    pub fn ensure_configured() {
        let secret = env::var("JWT_SECRET").unwrap_or_default();
        if let Err(problem) = check_secret(&secret) {
            panic!("JWT_SECRET {problem}. Generate one with `openssl rand -hex 32`.");
        }
    }
}

fn check_secret(secret: &str) -> std::result::Result<(), &'static str> {
    if secret.is_empty() {
        Err("is not set")
    } else if secret == "CHANGE_ME_IN_PROD" {
        Err("is still the placeholder value")
    } else if secret.len() < MIN_SECRET_LEN {
        Err("must be at least 32 characters")
    } else {
        Ok(())
    }
}

fn secret() -> Vec<u8> {
    env::var("JWT_SECRET")
        .expect("JWT_SECRET is not present")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::check_secret;

    #[test]
    fn rejects_missing_short_and_placeholder_secrets() {
        assert!(check_secret("").is_err());
        assert!(check_secret("CHANGE_ME_IN_PROD").is_err());
        assert!(check_secret("too-short").is_err());
        assert!(check_secret(&"a".repeat(32)).is_ok());
    }
}
