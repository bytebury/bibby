pub mod admin;
pub mod current_user;
pub mod maybe_current_user;
pub mod real_ip;

use crate::domain::user::User;
use crate::infra::auth::jwt::{Jwt, UserClaims};
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

pub enum BaseUser {
    User(Box<User>),
    None,
}

impl FromRequestParts<SharedState> for BaseUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(token) = jar.get("auth_token").map(|c| c.value()) else {
            return Ok(BaseUser::None);
        };

        if let Ok(data) = Jwt::verify::<UserClaims>(token)
            && let Ok(user) = User::find_by_email(state.db.as_ref(), &data.claims.sub).await
        {
            return Ok(BaseUser::User(Box::new(user)));
        }

        Ok(BaseUser::None)
    }
}
