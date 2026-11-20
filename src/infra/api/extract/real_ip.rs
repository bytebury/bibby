use crate::prelude::*;
use axum::extract::{ConnectInfo, FromRequestParts};
use std::net::{IpAddr, SocketAddr};

/// The client's IP address.
///
/// Behind Railway the socket address is its edge's, so the client comes from
/// `X-Real-IP`, which the edge sets to the address it received the request
/// from, replacing anything the client sent. Not `X-Forwarded-For`: the edge
/// appends its own hop to it (so the last entry is Railway's) and the client
/// chooses the first. Without the header (local runs) it's the socket.
#[derive(Clone)]
pub struct RealIp(pub String);

impl<S> FromRequestParts<S> for RealIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self> {
        let connect_info = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|err| AppError::Internal(format!("Unable to get IP Address: {err}")))?;
        let ip = parts
            .headers
            .get("x-real-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_ip)
            .unwrap_or_else(|| connect_info.ip());

        Ok(RealIp(ip.to_string()))
    }
}

/// The header's address, if it is a valid IP address (the value ends up in
/// several tables).
fn parse_ip(header: &str) -> Option<IpAddr> {
    header.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_ipv4_or_ipv6_address() {
        assert_eq!(
            parse_ip("203.0.113.7"),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            parse_ip(" 2001:db8::1 "),
            Some("2001:db8::1".parse().unwrap())
        );
    }

    #[test]
    fn ignores_anything_that_is_not_an_address() {
        assert_eq!(parse_ip(""), None);
        assert_eq!(parse_ip("unknown"), None);
        assert_eq!(parse_ip("8.8.8.8, 203.0.113.7"), None);
        assert_eq!(parse_ip(&"9".repeat(100_000)), None);
    }
}
