use crate::domain::sign_in::SignIn;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;

/// Daily job: drops sign-in history past the retention window (each user's
/// latest sign-in is always kept).
pub struct PruneSignInsUseCase {
    db: SharedDatabase,
}

impl PruneSignInsUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    pub async fn execute(&self) -> Result<()> {
        let removed = SignIn::prune(self.db.as_ref()).await?;
        info!(removed, "pruned old sign-ins");
        Ok(())
    }
}
