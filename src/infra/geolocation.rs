use crate::prelude::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration as StdDuration;
use std::time::Instant;

/// Every sign-in waits on this lookup, so a slow service must not hold it up
/// for long: past this, the sign-in goes ahead with an unknown location.
const LOOKUP_TIMEOUT: StdDuration = StdDuration::from_secs(2);

/// The geodude client for `GEODUDE_URL`, or `None` when it's unset or invalid.
fn client() -> Option<&'static geodude::Client> {
    static CLIENT: OnceLock<Option<geodude::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let url = env::var("GEODUDE_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())?;
            let http = reqwest::Client::builder()
                .timeout(LOOKUP_TIMEOUT)
                .build()
                .inspect_err(|e| warn!("could not build the geodude HTTP client: {e}"))
                .ok()?;
            geodude::Client::with_http(http, url)
                .inspect_err(|e| warn!("GEODUDE_URL is not usable: {e}"))
                .ok()
        })
        .as_ref()
}

#[derive(Debug, Clone)]
pub struct CountryDetails {
    pub name: String,
    pub code: String,
}

impl Default for CountryDetails {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            // Lowercase to match `countries.code` (stored LOWER) so the
            // /assets/images/flags/unknown.svg fallback resolves.
            code: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocationDetails {
    pub country: CountryDetails,
    pub region: Option<String>,
    pub city: Option<String>,
}

impl LocationDetails {
    /// Resolve an IP via the `geodude` crate (which talks to a `geodude`
    /// microservice, lazily initialized from `GEODUDE_URL`). Returns
    /// `Default` (Unknown) on any failure — missing env var, unparseable
    /// response, network error — so callers can `unwrap_or_default()` and
    /// continue with registration.
    pub async fn lookup(ip: IpAddr) -> Result<Self> {
        if let Some(cached) = cache().get(ip) {
            return Ok(cached);
        }
        let details = Self::fetch(ip).await?;
        cache().put(ip, details.clone());
        Ok(details)
    }

    async fn fetch(ip: IpAddr) -> Result<Self> {
        let client = client().ok_or(geodude::Error::NotConfigured)?;
        let location = client
            .locate(ip)
            .await
            .inspect_err(|e| warn!("Issue getting location: {}", e))?;
        let country = match (location.country_name, location.country_code) {
            (Some(name), Some(code)) if name != "-" && code != "-" => CountryDetails {
                name,
                code: code.to_lowercase(),
            },
            _ => CountryDetails::default(),
        };

        Ok(LocationDetails {
            country,
            region: clean(location.region),
            city: clean(location.city),
        })
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty() && v != "-")
}

/// Recent lookups by IP. A sign-up checks the visitor's location before it
/// creates the account and again while creating it; this keeps that (and a
/// burst of sign-ins from one address) to one call to the service.
struct LookupCache(Mutex<HashMap<IpAddr, (Instant, LocationDetails)>>);

const CACHE_TTL: StdDuration = StdDuration::from_secs(60);
const CACHE_CAPACITY: usize = 10_000;

fn cache() -> &'static LookupCache {
    static CACHE: LazyLock<LookupCache> = LazyLock::new(|| LookupCache(Mutex::default()));
    &CACHE
}

impl LookupCache {
    fn get(&self, ip: IpAddr) -> Option<LocationDetails> {
        let entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .get(&ip)
            .filter(|(at, _)| at.elapsed() < CACHE_TTL)
            .map(|(_, details)| details.clone())
    }

    fn put(&self, ip: IpAddr, details: LocationDetails) {
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if entries.len() >= CACHE_CAPACITY {
            entries.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
        }
        entries.insert(ip, (Instant::now(), details));
    }
}
