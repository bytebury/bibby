use crate::domain::user::User;
use crate::infra::api::extract::session_user;
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct MaybeCurrentUser(pub Option<User>);

impl FromRequestParts<SharedState> for MaybeCurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        Ok(MaybeCurrentUser(session_user(parts, state).await?))
    }
}
