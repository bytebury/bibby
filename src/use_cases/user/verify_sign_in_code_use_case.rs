use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::user::User;
use crate::domain::value_objects::admin_emails::AdminEmails;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;
use std::sync::Arc;

/// Exchanges an emailed sign-in code for the account it was sent to,
/// verifying the account if this is the first code it has used. The caller
/// still owns minting the session and re-checking locks.
pub struct VerifySignInCodeUseCase {
    db: SharedDatabase,
    admins: Arc<AdminEmails>,
}

impl VerifySignInCodeUseCase {
    pub fn new(db: SharedDatabase, admins: Arc<AdminEmails>) -> Self {
        Self { db, admins }
    }

    pub async fn execute(&self, email: &Email, code: &str) -> Result<User> {
        let code = OneTimeCode::parse(code)?;

        // Every failure reads the same. Saying how many tries are left would
        // tell a stranger the address has an account, since unknown addresses
        // never get a code to try.
        let Some(passcode) = OneTimePasscode::claim_attempt(self.db.as_ref(), email).await? else {
            return Err(rejected());
        };
        if !code.matches(email, &passcode.code_hash) || !passcode.consume(self.db.as_ref()).await? {
            return Err(rejected());
        }

        // Typing in a code we emailed proves they own the address, which is
        // all verification asks of an email sign-up.
        let mut user = User::find_by_email(self.db.as_ref(), email.as_str()).await?;
        if !user.verified {
            User::mark_verified(self.db.as_ref(), user.id).await?;
            user = User::find_by_id(self.db.as_ref(), user.id).await?;
        }
        user.grant_listed_admin(self.db.as_ref(), &self.admins)
            .await
    }
}

fn rejected() -> AppError {
    AppError::BadRequest(
        "That code is wrong or has expired. Check it, or request a new one.".to_string(),
    )
}
