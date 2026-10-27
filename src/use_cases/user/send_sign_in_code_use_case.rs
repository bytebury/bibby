use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::user::User;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::infra::db::SharedDatabase;
use crate::infra::email;
use crate::prelude::*;

/// Emails a one-time sign-in code to an existing account. It is how email
/// sign-ups always sign in (and how they verify their address the first
/// time), and the fallback for Google accounts when Google is unavailable.
///
/// Every outcome looks identical to the caller: unknown addresses, locked
/// accounts, and throttled requests all return `Ok(())` without sending
/// anything, so the form can't be used to discover who has an account.
pub struct SendSignInCodeUseCase {
    db: SharedDatabase,
}

impl SendSignInCodeUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    pub async fn execute(&self, email: &Email) -> Result<()> {
        let Ok(user) = User::find_by_email(self.db.as_ref(), email.as_str()).await else {
            return Ok(());
        };
        if user.locked {
            return Ok(());
        }

        let code = OneTimeCode::generate();
        let mut tx = self.db.begin().await?;
        OneTimePasscode::prune(&mut *tx).await?;
        if !OneTimePasscode::can_issue(&mut *tx, email).await? {
            return Ok(());
        }
        let passcode = OneTimePasscode::create(&mut *tx, email, &code).await?;
        tx.commit().await?;

        if let Err(error) =
            email::send_sign_in_code(email, &user.first_name, &code, !user.verified).await
        {
            // They never received it, so it shouldn't hold up a retry.
            error!(%error, email = %email, "could not send sign-in code");
            OneTimePasscode::delete(self.db.as_ref(), passcode.id).await?;
        }

        Ok(())
    }
}
