//! What every page shows that admins change at runtime: the active
//! announcement banner and the feature toggles. Read from the database for
//! each page, so every server shows a change the moment it is saved.

use crate::domain::announcement::Announcement;
use crate::domain::feature_toggle::{FeatureToggle, FeatureToggles};
use crate::infra::db::DbPool;
use crate::prelude::*;

#[derive(Debug, Default)]
pub struct SiteSnapshot {
    pub banner: Option<Announcement>,
    pub toggles: FeatureToggles,
}

impl SiteSnapshot {
    /// Both queries run at once, so a page waits for one round trip. If the
    /// database fails, the page still renders: no banner, every toggle off.
    pub async fn load(db: &DbPool) -> Self {
        let (banner, toggles) =
            tokio::join!(Announcement::find_latest_active(db), FeatureToggle::all(db),);
        Self {
            banner: banner
                .inspect_err(|error| warn!(%error, "could not load the banner"))
                .unwrap_or_default(),
            toggles: FeatureToggles::new(
                toggles
                    .inspect_err(|error| warn!(%error, "could not load feature toggles"))
                    .unwrap_or_default(),
            ),
        }
    }
}
