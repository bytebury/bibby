use crate::domain::Access;
use crate::domain::user::User;
use crate::prelude::*;

/// A feature an admin can switch on at `/admin/feature-toggles`. Each one has
/// a row in `feature_toggles` (added by a migration) holding whether it is on
/// and its value, if it takes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// Loads Google Analytics on every page, measuring into `value`.
    GoogleAnalytics,
}

impl Feature {
    pub const ALL: [Feature; 1] = [Feature::GoogleAnalytics];

    /// The row's `key` column.
    pub fn key(self) -> &'static str {
        match self {
            Feature::GoogleAnalytics => "google_analytics",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|feature| feature.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            Feature::GoogleAnalytics => "Google Analytics",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Feature::GoogleAnalytics => {
                "Loads Google Analytics on every page. Mention it in your privacy policy."
            }
        }
    }

    /// The label for the value field, or `None` if the feature takes no value.
    pub fn value_label(self) -> Option<&'static str> {
        match self {
            Feature::GoogleAnalytics => Some("Measurement ID (e.g. G-XXXXXXXXXX)"),
        }
    }

    /// The value to store, parsed from the form. A feature that takes a value
    /// can't be turned on without one.
    pub fn parse_value(self, enabled: bool, raw: &str) -> Result<Option<String>> {
        let raw = raw.trim();
        let value = match self {
            Feature::GoogleAnalytics if raw.is_empty() => None,
            // Rendered into a <script> tag, so only the ID's own characters
            // are let through.
            Feature::GoogleAnalytics => {
                let tag = raw.to_ascii_uppercase();
                if !is_google_tag(&tag) {
                    return Err(AppError::BadRequest(
                        "Enter a Google tag ID like G-XXXXXXXXXX.".to_string(),
                    ));
                }
                Some(tag)
            }
        };
        if enabled && value.is_none() && self.value_label().is_some() {
            return Err(AppError::BadRequest(format!(
                "{} needs a value before it can be turned on.",
                self.name()
            )));
        }
        Ok(value)
    }
}

/// `G-…` (GA4), `GT-…`, `AW-…` (Ads) or `DC-…`, then letters and digits.
fn is_google_tag(tag: &str) -> bool {
    let Some((prefix, id)) = tag.split_once('-') else {
        return false;
    };
    matches!(prefix, "G" | "GT" | "AW" | "DC")
        && (4..=20).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[derive(Debug, Clone, FromRow)]
pub struct FeatureToggle {
    pub id: PrimaryKey,
    pub key: String,
    pub enabled: bool,
    pub value: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl FeatureToggle {
    pub fn feature(&self) -> Option<Feature> {
        Feature::from_key(&self.key)
    }

    /// Every row the code knows a feature for, in `Feature::ALL` order. Rows
    /// for features since removed from the code are skipped.
    pub async fn all<'e, E>(exec: E) -> Result<Vec<FeatureToggle>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let mut toggles = sqlx::query_as!(
            FeatureToggle,
            "SELECT id, key, enabled, value, updated_at FROM feature_toggles"
        )
        .fetch_all(exec)
        .await?;
        toggles.retain(|toggle| toggle.feature().is_some());
        toggles.sort_by_key(|toggle| {
            Feature::ALL
                .iter()
                .position(|f| Some(*f) == toggle.feature())
        });
        Ok(toggles)
    }

    pub async fn find_by_id<'e, E>(exec: E, id: PrimaryKey) -> Result<FeatureToggle>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let toggle = sqlx::query_as!(
            FeatureToggle,
            "SELECT id, key, enabled, value, updated_at FROM feature_toggles WHERE id = $1",
            id
        )
        .fetch_one(exec)
        .await?;
        Ok(toggle)
    }

    pub async fn update<'e, E>(
        exec: E,
        id: PrimaryKey,
        enabled: bool,
        value: Option<&str>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!(
            "UPDATE feature_toggles SET enabled = $1, value = $2 WHERE id = $3",
            enabled,
            value,
            id
        )
        .execute(exec)
        .await?;
        Ok(())
    }
}

/// Site settings: anyone may read them, only admins may change them.
impl Access<FeatureToggle> for User {
    fn can_write(&self, _: &FeatureToggle) -> bool {
        self.is_admin()
    }
}

/// The toggles as the pages see them. Kept in memory (see `infra::site`).
#[derive(Debug, Clone, Default)]
pub struct FeatureToggles(Vec<FeatureToggle>);

impl FeatureToggles {
    pub fn new(toggles: Vec<FeatureToggle>) -> Self {
        Self(toggles)
    }

    /// The feature's value when it is on, e.g. the measurement ID.
    pub fn value(&self, feature: Feature) -> Option<&str> {
        self.0
            .iter()
            .find(|toggle| toggle.enabled && toggle.feature() == Some(feature))
            .and_then(|toggle| toggle.value.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toggle(enabled: bool, value: Option<&str>) -> FeatureToggle {
        FeatureToggle {
            id: 1,
            key: Feature::GoogleAnalytics.key().to_string(),
            enabled,
            value: value.map(str::to_string),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_google_tag_ids_and_normalises_case() {
        let ga = Feature::GoogleAnalytics;
        assert_eq!(
            ga.parse_value(true, " g-ab12cd34ef ").unwrap().as_deref(),
            Some("G-AB12CD34EF")
        );
        assert_eq!(
            ga.parse_value(true, "AW-123456789").unwrap().as_deref(),
            Some("AW-123456789")
        );
    }

    #[test]
    fn rejects_anything_that_could_break_out_of_the_script() {
        let ga = Feature::GoogleAnalytics;
        for raw in [
            "G-ABC'); alert(1); //",
            "G-AB CD",
            "UA-12345-1",
            "X-ABCDEF",
            "G-",
            "GABCDEF",
        ] {
            assert!(ga.parse_value(false, raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn a_feature_with_a_value_cannot_be_turned_on_without_one() {
        let ga = Feature::GoogleAnalytics;
        assert!(ga.parse_value(true, "  ").is_err());
        assert_eq!(ga.parse_value(false, "").unwrap(), None);
    }

    #[test]
    fn a_value_only_counts_while_the_toggle_is_on() {
        let on = FeatureToggles::new(vec![toggle(true, Some("G-AB12CD34EF"))]);
        let off = FeatureToggles::new(vec![toggle(false, Some("G-AB12CD34EF"))]);
        assert_eq!(on.value(Feature::GoogleAnalytics), Some("G-AB12CD34EF"));
        assert_eq!(off.value(Feature::GoogleAnalytics), None);
        assert_eq!(
            FeatureToggles::default().value(Feature::GoogleAnalytics),
            None
        );
    }

    #[test]
    fn every_feature_round_trips_through_its_key() {
        for feature in Feature::ALL {
            assert_eq!(Feature::from_key(feature.key()), Some(feature));
        }
        assert_eq!(Feature::from_key("removed_feature"), None);
    }
}
