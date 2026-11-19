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
            user.verified = true;
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

#[cfg(test)]
mod db_tests {
    use super::*;
    use crate::domain::user::Role;
    use crate::test_support::insert_user;
    use sqlx::PgPool;

    fn verifier(db: PgPool) -> VerifySignInCodeUseCase {
        let admins = AdminEmails::parse("boss@example.com").unwrap();
        VerifySignInCodeUseCase::new(Arc::new(db), Arc::new(admins))
    }

    async fn issue(db: &PgPool, email: &Email) -> OneTimeCode {
        let code = OneTimeCode::generate();
        OneTimePasscode::create(db, email, &code).await.unwrap();
        code
    }

    #[sqlx::test]
    async fn a_listed_address_becomes_an_admin_once_it_enters_its_code(db: PgPool) {
        let email = Email::parse("boss@example.com").unwrap();
        insert_user(&db, email.as_str(), Role::Free, "10.0.1.1").await;
        let code = issue(&db, &email).await;
        let user = verifier(db).execute(&email, code.as_str()).await.unwrap();
        assert!(user.is_admin());
    }

    #[sqlx::test]
    async fn an_unlisted_address_stays_free(db: PgPool) {
        let email = Email::parse("someone@example.com").unwrap();
        insert_user(&db, email.as_str(), Role::Free, "10.0.1.2").await;
        let code = issue(&db, &email).await;
        let user = verifier(db).execute(&email, code.as_str()).await.unwrap();
        assert!(user.is_free());
    }

    #[sqlx::test]
    async fn a_wrong_code_reads_the_same_as_an_unknown_address(db: PgPool) {
        let known = Email::parse("known@example.com").unwrap();
        insert_user(&db, known.as_str(), Role::Free, "10.0.1.3").await;
        let code = issue(&db, &known).await;
        let wrong = if code.as_str() == "000000" {
            "111111"
        } else {
            "000000"
        };
        let verifier = verifier(db);

        let for_known = verifier
            .execute(&known, wrong)
            .await
            .unwrap_err()
            .to_string();
        let unknown = Email::parse("nobody@example.com").unwrap();
        let for_unknown = verifier
            .execute(&unknown, wrong)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(for_known, for_unknown);
    }

    #[sqlx::test]
    async fn a_code_works_once_and_not_after_too_many_guesses(db: PgPool) {
        let email = Email::parse("guesser@example.com").unwrap();
        insert_user(&db, email.as_str(), Role::Free, "10.0.1.4").await;
        let verifier = verifier(db.clone());

        let code = issue(&db, &email).await;
        verifier.execute(&email, code.as_str()).await.unwrap();
        assert!(
            verifier.execute(&email, code.as_str()).await.is_err(),
            "no replay"
        );

        let code = issue(&db, &email).await;
        let wrong = if code.as_str() == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..crate::domain::one_time_passcode::MAX_ATTEMPTS {
            assert!(verifier.execute(&email, wrong).await.is_err());
        }
        assert!(
            verifier.execute(&email, code.as_str()).await.is_err(),
            "out of attempts"
        );
    }
}
