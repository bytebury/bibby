use crate::domain::user::User;
use crate::infra::api::extract::BaseUser;
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct CurrentUser(pub User);

impl FromRequestParts<SharedState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        match BaseUser::from_request_parts(parts, state).await? {
            BaseUser::User(user) => Ok(CurrentUser(*user)),
            _ => Err(AppError::Unauthorized),
        }
    }
}
