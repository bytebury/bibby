use crate::prelude::*;
use jsonwebtoken::*;

pub mod user_claims;

pub use user_claims::UserClaims;

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

fn secret() -> Vec<u8> {
    env::var("JWT_SECRET")
        .expect("JWT_SECRET is not present")
        .into_bytes()
}
