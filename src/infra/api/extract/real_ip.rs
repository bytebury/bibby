use crate::prelude::*;
use axum::extract::{ConnectInfo, FromRequestParts};
use std::net::SocketAddr;
use std::sync::Arc;

/// The client's IP address.
///
/// Behind a reverse proxy the socket address is the proxy's, so the client
/// comes from `X-Forwarded-For`. Each proxy appends the address it received
/// the request from, and the client can put anything it likes in front, so
/// only the entries our own proxies appended are trusted: the one
/// `TRUSTED_PROXY_HOPS` (default 1) from the right. Set it to the number of
/// proxies in front of the app, or 0 when nothing is, to ignore the header.
#[derive(Clone)]
pub struct RealIp(pub String);

impl<S> FromRequestParts<Arc<S>> for RealIp
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &Arc<S>,
    ) -> Result<Self> {
        let connect_info = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|err| AppError::Internal(format!("Unable to get IP Address: {err}")))?;
        let forwarded = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok());
        let ip = forwarded
            .and_then(|header| client_from_forwarded(header, trusted_proxy_hops()))
            .unwrap_or_else(|| connect_info.ip().to_string());

        Ok(RealIp(ip))
    }
}

fn trusted_proxy_hops() -> usize {
    env::var("TRUSTED_PROXY_HOPS")
        .ok()
        .and_then(|hops| hops.trim().parse().ok())
        .unwrap_or(1)
}

/// The entry `hops` from the right of an `X-Forwarded-For` header, or `None`
/// when no proxy is trusted or the header is shorter than the proxy chain.
fn client_from_forwarded(header: &str, hops: usize) -> Option<String> {
    if hops == 0 {
        return None;
    }
    let entries: Vec<&str> = header.split(',').map(str::trim).collect();
    let index = entries.len().checked_sub(hops)?;
    entries
        .get(index)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_cannot_choose_its_ip_by_prepending_entries() {
        let header = "8.8.8.8, 203.0.113.7";
        assert_eq!(
            client_from_forwarded(header, 1).as_deref(),
            Some("203.0.113.7")
        );
    }

    #[test]
    fn each_trusted_proxy_moves_one_entry_left() {
        let header = "8.8.8.8, 203.0.113.7, 172.16.0.1";
        assert_eq!(
            client_from_forwarded(header, 2).as_deref(),
            Some("203.0.113.7")
        );
    }

    #[test]
    fn no_trusted_proxy_or_a_short_header_falls_back_to_the_socket() {
        assert_eq!(client_from_forwarded("203.0.113.7", 0), None);
        assert_eq!(client_from_forwarded("203.0.113.7", 2), None);
        assert_eq!(client_from_forwarded("", 1), None);
    }
}
