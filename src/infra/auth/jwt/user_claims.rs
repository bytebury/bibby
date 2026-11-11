use crate::domain::user::User;
use crate::prelude::*;

/// How long a sign-in lasts. The JWT's `exp` and the cookie's `Max-Age` both use
/// it, so the browser keeps the cookie exactly as long as the token is valid.
pub const SESSION_DAYS: i64 = 365;

#[derive(Debug, Deserialize, Serialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
}

impl From<User> for UserClaims {
    fn from(user: User) -> Self {
        let exp = Utc::now()
            .checked_add_signed(Duration::days(SESSION_DAYS))
            .expect("valid timestamp")
            .timestamp() as usize;

        Self {
            sub: user.email,
            exp,
        }
    }
}
