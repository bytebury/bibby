use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::sign_in::SignIn;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;

/// Daily job: drops sign-in history past the retention window (each user's
/// latest sign-in is always kept) and day-old sign-in codes.
pub struct PruneSignInsUseCase {
    db: SharedDatabase,
}

impl PruneSignInsUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    pub async fn execute(&self) -> Result<()> {
        let removed = SignIn::prune(self.db.as_ref()).await?;
        // Code rows only matter for the hourly cap, so they go here too.
        OneTimePasscode::prune(self.db.as_ref()).await?;
        info!(removed, "pruned old sign-ins and sign-in codes");
        Ok(())
    }
}
