use crate::domain::user::User;
use crate::infra::api::extract::session_user;
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct Admin(pub User);

impl FromRequestParts<SharedState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        match session_user(parts, state).await? {
            Some(user) if user.is_admin() => Ok(Admin(user)),
            _ => Err(AppError::Forbidden(
                "You do not have access to view this page, or this page does not exist.".into(),
            )),
        }
    }
}
