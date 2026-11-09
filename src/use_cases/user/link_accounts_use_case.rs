use crate::domain::account_group::AccountGroup;
use crate::domain::user::User;
use crate::domain::value_objects::email::Email;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;

/// Admin-only: marks two accounts as belonging to the same person.
pub struct LinkAccountsUseCase {
    db: SharedDatabase,
}

impl LinkAccountsUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    /// Links `user` with the account at `email`. Linking an account that's
    /// already in the same group is a no-op.
    pub async fn execute(&self, admin: &User, user: &User, email: &Email) -> Result<User> {
        admin.ensure_can_write(user)?;
        let other = User::find_by_email(self.db.as_ref(), email.as_str())
            .await
            .map_err(|_| AppError::NotFound("No account uses that email address.".into()))?;
        if other.id == user.id {
            return Err(AppError::BadRequest(
                "An account can't be linked to itself.".into(),
            ));
        }

        let mut tx = self.db.begin().await?;
        AccountGroup::link(&mut tx, user.id, other.id, admin.id).await?;
        tx.commit().await?;
        Ok(other)
    }
}
