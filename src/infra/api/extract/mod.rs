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

/// The signed-in user behind the request's `auth_token` cookie, or `None`.
/// The root of `CurrentUser`, `MaybeCurrentUser` and `Admin`.
///
/// Verifies the JWT and runs the per-request audit. A locked account, one that
/// hasn't verified its email, or a token for an older session (see
/// `User::end_all_sessions`) counts as signed out. The audit still records a
/// locked account's IP, so the shared-IP lock keeps propagating.
pub async fn session_user(parts: &mut Parts, state: &SharedState) -> Result<Option<User>> {
    let jar = CookieJar::from_headers(&parts.headers);
    let Some(token) = jar.get("auth_token").map(|c| c.value().to_string()) else {
        return Ok(None);
    };
    let Ok(data) = Jwt::verify::<UserClaims>(&token) else {
        return Ok(None);
    };
    let RealIp(ip) = RealIp::from_request_parts(parts, state).await?;
    let Ok(user) = User::audit_and_find_by_email(state.db.as_ref(), &data.claims.sub, &ip).await
    else {
        return Ok(None);
    };
    let signed_in = !user.locked && user.verified && data.claims.is_current_for(&user);
    Ok(signed_in.then_some(user))
}
