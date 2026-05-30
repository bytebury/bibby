use crate::domain::user::User;
use crate::prelude::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
}

impl From<User> for UserClaims {
    fn from(user: User) -> Self {
        let exp = Utc::now()
            .checked_add_signed(Duration::days(365))
            .expect("valid timestamp")
            .timestamp() as usize;

        Self {
            sub: user.email,
            exp,
        }
    }
}
