use crate::domain::account_group::AccountGroup;
use crate::domain::user::User;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;

/// Admin-only: takes an account out of the group it was linked into.
pub struct UnlinkAccountUseCase {
    db: SharedDatabase,
}

impl UnlinkAccountUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    /// Unlinks `other_id` from `user`'s group. `other_id` must actually be
    /// linked to `user`, so an id from the URL can't reach an unrelated group.
    pub async fn execute(&self, admin: &User, user: &User, other_id: PrimaryKey) -> Result<()> {
        admin.ensure_can_write(user)?;
        let mut tx = self.db.begin().await?;
        let linked = AccountGroup::linked_accounts(&mut *tx, user.id).await?;
        if !linked.iter().any(|u| u.id == other_id) {
            return Err(AppError::NotFound(
                "That account isn't linked to this one.".into(),
            ));
        }
        AccountGroup::unlink(&mut tx, other_id).await?;
        tx.commit().await?;
        Ok(())
    }
}
