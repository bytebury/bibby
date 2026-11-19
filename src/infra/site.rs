//! What every page shows that admins change at runtime: the active
//! announcement banner and the feature toggles.
//!
//! Every render needs them, so they're held in memory rather than queried per
//! request. They load at startup, reload right after an admin change on this
//! server, and refresh every `REFRESH` so other replicas catch up too.

use crate::domain::announcement::Announcement;
use crate::domain::feature_toggle::{FeatureToggle, FeatureToggles};
use crate::infra::db::SharedDatabase;
use crate::prelude::*;
use std::sync::{Arc, RwLock};
use std::time::Duration as StdDuration;

const REFRESH: StdDuration = StdDuration::from_secs(30);

#[derive(Debug, Default)]
pub struct SiteSnapshot {
    pub banner: Option<Announcement>,
    pub toggles: FeatureToggles,
}

pub struct SiteCache {
    db: SharedDatabase,
    snapshot: RwLock<Arc<SiteSnapshot>>,
}

impl SiteCache {
    /// Loads the snapshot and starts the background refresh.
    pub async fn start(db: SharedDatabase) -> Arc<Self> {
        let cache = Arc::new(Self {
            db,
            snapshot: RwLock::default(),
        });
        cache.reload().await;
        let refresher = Arc::clone(&cache);
        tokio::spawn(async move {
            let mut every = tokio::time::interval(REFRESH);
            every.tick().await;
            loop {
                every.tick().await;
                refresher.reload().await;
            }
        });
        cache
    }

    pub fn get(&self) -> Arc<SiteSnapshot> {
        Arc::clone(&self.snapshot.read().unwrap_or_else(|e| e.into_inner()))
    }

    /// Re-reads both from the database. On an error the previous snapshot is
    /// kept, so a database blip doesn't blank the banner or turn off analytics.
    pub async fn reload(&self) {
        let (banner, toggles) = tokio::join!(
            Announcement::find_latest_active(self.db.as_ref()),
            FeatureToggle::all(self.db.as_ref()),
        );
        match (banner, toggles) {
            (Ok(banner), Ok(toggles)) => {
                let snapshot = Arc::new(SiteSnapshot {
                    banner,
                    toggles: FeatureToggles::new(toggles),
                });
                *self.snapshot.write().unwrap_or_else(|e| e.into_inner()) = snapshot;
            }
            (Err(error), _) | (_, Err(error)) => {
                warn!(%error, "could not refresh the banner and feature toggles");
            }
        }
    }
}
