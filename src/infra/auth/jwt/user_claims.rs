use crate::domain::user::User;
use crate::prelude::*;

/// How long a sign-in lasts. The JWT's `exp` and the cookie's `Max-Age` both use
/// it, so the browser keeps the cookie exactly as long as the token is valid.
pub const SESSION_DAYS: i64 = 365;

#[derive(Debug, Deserialize, Serialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
    /// The account the token was minted for. An address can move to another
    /// account (an admin edit), so `sub` alone could match the wrong one.
    /// Absent from tokens minted before it existed.
    #[serde(default)]
    pub uid: Option<PrimaryKey>,
    /// The account's `session_version` when the token was minted.
    #[serde(default)]
    pub ver: i32,
}

impl UserClaims {
    /// Whether this token still signs in as `user`.
    pub fn is_current_for(&self, user: &User) -> bool {
        self.ver == user.session_version && self.uid.is_none_or(|uid| uid == user.id)
    }
}

impl From<&User> for UserClaims {
    fn from(user: &User) -> Self {
        let exp = Utc::now()
            .checked_add_signed(Duration::days(SESSION_DAYS))
            .expect("valid timestamp")
            .timestamp() as usize;

        Self {
            uid: Some(user.id),
            ver: user.session_version,
            sub: user.email.clone(),
            exp,
        }
    }
}
