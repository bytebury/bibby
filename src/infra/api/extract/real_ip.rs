use crate::prelude::*;
use axum::extract::{ConnectInfo, FromRequestParts};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct RealIp(pub String);

impl<S> FromRequestParts<Arc<S>> for RealIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &Arc<S>,
    ) -> Result<Self> {
        let connect_info = ConnectInfo::<SocketAddr>::from_request_parts(parts, _state)
            .await
            .map_err(|err| AppError::Internal(format!("Unable to get IP Address: {err}")))?;
        let ip = if let Some(forwarded) = parts.headers.get("x-forwarded-for") {
            match forwarded.to_str() {
                Ok(forwarded) => forwarded
                    .split(',')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                Err(_) => connect_info.ip().to_string(),
            }
        } else {
            connect_info.ip().to_string()
        };

        Ok(RealIp(ip))
    }
}
