use crate::domain::user::CreateUser;
use crate::error::Result;

pub mod google;
pub mod jwt;
pub mod oauth_state;

pub trait OAuthProvider {
    fn get_auth_url(&self) -> String;
    fn exchange_code_for_user(&self, code: &str) -> impl Future<Output = Result<CreateUser>>;
}
