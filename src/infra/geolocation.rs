use crate::prelude::*;
use std::net::IpAddr;

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
}

impl LocationDetails {
    /// Resolve an IP via the `geodude` crate (which talks to a `geodude`
    /// microservice, lazily initialized from `GEODUDE_URL`). Returns
    /// `Default` (Unknown) on any failure — missing env var, unparseable
    /// response, network error — so callers can `unwrap_or_default()` and
    /// continue with registration.
    pub async fn lookup(ip: IpAddr) -> Result<Self> {
        let location = geodude::locate(ip).await?;
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
        })
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty() && v != "-")
}
