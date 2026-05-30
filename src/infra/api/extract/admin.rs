use crate::domain::user::User;
use crate::infra::api::extract::BaseUser;
use crate::prelude::*;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct Admin(pub User);

impl FromRequestParts<SharedState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self> {
        match BaseUser::from_request_parts(parts, state).await? {
            BaseUser::User(user) if user.is_admin() => Ok(Admin(*user)),
            _ => Err(AppError::Forbidden(
                "You do not have access to view this page, or this page does not exist.".into(),
            )),
        }
    }
}
