use crate::domain::one_time_passcode::OneTimePasscode;
use crate::domain::user::User;
use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::infra::db::SharedDatabase;
use crate::prelude::*;

/// Exchanges an emailed sign-in code for the account it was sent to,
/// verifying the account if this is the first code it has used. The caller
/// still owns minting the session and re-checking locks.
pub struct VerifySignInCodeUseCase {
    db: SharedDatabase,
}

impl VerifySignInCodeUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    pub async fn execute(&self, email: &Email, code: &str) -> Result<User> {
        let code = OneTimeCode::parse(code)?;

        let Some(passcode) = OneTimePasscode::claim_attempt(self.db.as_ref(), email).await? else {
            return Err(expired());
        };

        if !code.matches(email, &passcode.code_hash) {
            return Err(match passcode.attempts_remaining() {
                0 => AppError::BadRequest(
                    "That code isn't right, and it has been tried too many times. Request a new one."
                        .to_string(),
                ),
                1 => AppError::BadRequest("That code isn't right. You have 1 try left.".to_string()),
                n => AppError::BadRequest(format!("That code isn't right. You have {n} tries left.")),
            });
        }

        if !passcode.consume(self.db.as_ref()).await? {
            return Err(expired());
        }

        // Typing in a code we emailed proves they own the address, which is
        // all verification asks of an email sign-up.
        let user = User::find_by_email(self.db.as_ref(), email.as_str()).await?;
        if user.verified {
            return Ok(user);
        }
        User::mark_verified(self.db.as_ref(), user.id).await?;
        User::find_by_id(self.db.as_ref(), user.id).await
    }
}

fn expired() -> AppError {
    AppError::BadRequest(
        "That code has expired or been used too many times. Request a new one.".to_string(),
    )
}
