use crate::domain::value_objects::email::Email;
use crate::domain::value_objects::one_time_code::OneTimeCode;
use crate::prelude::*;

/// How long an emailed code stays valid.
pub const TTL_MINUTES: i64 = 10;
/// Wrong guesses allowed against a single code before it is burned.
pub const MAX_ATTEMPTS: i32 = 5;
/// Minimum gap between two codes sent to the same address.
pub const RESEND_COOLDOWN_SECONDS: i64 = 60;
/// Codes sent to one address per rolling hour. Together with `MAX_ATTEMPTS`
/// this caps an attacker at 25 guesses an hour against a million codes.
pub const MAX_ISSUED_PER_HOUR: i64 = 5;

/// An emailed sign-in code. Only the newest code for an address is ever
/// accepted, so requesting a new one silently retires the previous one.
#[derive(Debug, Clone, FromRow)]
pub struct OneTimePasscode {
    pub id: PrimaryKey,
    pub email: String,
    pub code_hash: Vec<u8>,
    pub attempts: i32,
}

impl OneTimePasscode {
    pub async fn create<'e, E>(exec: E, email: &Email, code: &OneTimeCode) -> Result<Self>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let expires_at = Utc::now() + Duration::minutes(TTL_MINUTES);
        let passcode = sqlx::query_as!(
            OneTimePasscode,
            r#"
            INSERT INTO one_time_passcodes (email, code_hash, expires_at)
            VALUES ($1, $2, $3)
            RETURNING id, email, code_hash, attempts
            "#,
            email.as_str(),
            code.digest(email),
            expires_at,
        )
        .fetch_one(exec)
        .await?;
        Ok(passcode)
    }

    /// Whether another code may be sent to `email` right now, given the
    /// resend cooldown and the hourly cap.
    pub async fn can_issue<'e, E>(exec: E, email: &Email) -> Result<bool>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*) AS "issued!",
                   COALESCE(BOOL_OR(created_at > NOW() - make_interval(secs => $2)), FALSE) AS "cooling_down!"
              FROM one_time_passcodes
             WHERE email = $1
               AND created_at > NOW() - INTERVAL '1 hour'
            "#,
            email.as_str(),
            RESEND_COOLDOWN_SECONDS as f64,
        )
        .fetch_one(exec)
        .await?;
        Ok(!row.cooling_down && row.issued < MAX_ISSUED_PER_HOUR)
    }

    /// Spends one attempt on the newest live code for `email` and returns it
    /// for comparison. Counting the attempt *before* comparing means parallel
    /// guesses can never exceed `MAX_ATTEMPTS`. `None` when there is no code,
    /// it has expired, or its attempts are used up.
    pub async fn claim_attempt<'e, E>(exec: E, email: &Email) -> Result<Option<Self>>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let passcode = sqlx::query_as!(
            OneTimePasscode,
            r#"
            UPDATE one_time_passcodes
               SET attempts = attempts + 1
             WHERE id = (SELECT id
                           FROM one_time_passcodes
                          WHERE email = $1
                          ORDER BY created_at DESC, id DESC
                          LIMIT 1)
               AND expires_at > NOW()
               AND attempts < $2
            RETURNING id, email, code_hash, attempts
            "#,
            email.as_str(),
            MAX_ATTEMPTS,
        )
        .fetch_optional(exec)
        .await?;
        Ok(passcode)
    }

    /// Burns this code — and every other code for the address, so someone who
    /// has proven they own it starts the hourly cap afresh. Returns `false` if
    /// a parallel request already consumed it.
    pub async fn consume<'e, E>(&self, exec: E) -> Result<bool>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        let deleted = sqlx::query_scalar!(
            "DELETE FROM one_time_passcodes WHERE email = $1 RETURNING id",
            self.email,
        )
        .fetch_all(exec)
        .await?;
        Ok(deleted.contains(&self.id))
    }

    pub async fn delete<'e, E>(exec: E, id: PrimaryKey) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!("DELETE FROM one_time_passcodes WHERE id = $1", id)
            .execute(exec)
            .await?;
        Ok(())
    }

    /// Rows only matter for the hourly cap, so anything older than a day is
    /// dead weight.
    pub async fn prune<'e, E>(exec: E) -> Result<()>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query!("DELETE FROM one_time_passcodes WHERE created_at < NOW() - INTERVAL '1 day'")
            .execute(exec)
            .await?;
        Ok(())
    }

    pub fn attempts_remaining(&self) -> i32 {
        (MAX_ATTEMPTS - self.attempts).max(0)
    }
}
