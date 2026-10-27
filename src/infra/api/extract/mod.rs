pub mod admin;
pub mod current_user;
pub mod maybe_current_user;
pub mod real_ip;
pub mod validated_form;

use crate::domain::user::User;
use crate::infra::api::extract::real_ip::RealIp;
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
        let RealIp(ip) = RealIp::from_request_parts(parts, state).await?;

        if let Ok(data) = Jwt::verify::<UserClaims>(token)
            && let Ok(user) =
                User::audit_and_find_by_email(state.db.as_ref(), &data.claims.sub, &ip).await
        {
            // A locked account with a still-valid cookie is treated as signed
            // out everywhere (the audit folded into the lookup above still
            // records their IP so the shared-IP lock keeps propagating). So is
            // one that hasn't verified its email —
            // sign-in never mints a session for those, so a cookie for one
            // shouldn't exist anyway.
            if user.locked || !user.verified {
                return Ok(BaseUser::None);
            }
            return Ok(BaseUser::User(Box::new(user)));
        }

        Ok(BaseUser::None)
    }
}
