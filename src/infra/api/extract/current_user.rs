use crate::domain::user::User;
use crate::infra::api::extract::session_user;
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct CurrentUser(pub User);

impl FromRequestParts<SharedState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        session_user(parts, state)
            .await?
            .map(CurrentUser)
            .ok_or(AppError::Unauthorized)
    }
}
