use crate::infra::db::SharedDatabase;
use crate::infra::email;
use crate::prelude::*;
use chrono::NaiveDate;

/// How many days before the expiry date each reminder goes out, ordered from
/// the earliest warning to the last one. A user only ever receives the
/// reminder for the largest threshold they have not passed yet, so an admin
/// who grants a five-day plan gets the one-week note and then the one-day
/// note — never a backlog of all three at once.
const REMINDER_THRESHOLDS: [i64; 3] = [30, 7, 1];

pub struct ProcessPlanExpirationsUseCase {
    db: SharedDatabase,
}

/// The subset of a user the job needs.
struct ExpiringUser {
    id: PrimaryKey,
    first_name: String,
    email: String,
    plan_expiry_date: NaiveDate,
    plan_expiry_reminder_days: Option<i32>,
}

impl ProcessPlanExpirationsUseCase {
    pub fn new(db: SharedDatabase) -> Self {
        Self { db }
    }

    /// Runs once per scheduler tick: first downgrade everyone whose
    /// admin-granted plan has lapsed, then warn everyone approaching one. The
    /// two sets are disjoint (lapsed is strictly in the past, reminders are
    /// strictly today or later), so the ordering only affects which log line
    /// comes first.
    pub async fn execute(&self) -> Result<()> {
        self.downgrade_expired().await?;
        self.send_reminders().await?;
        Ok(())
    }

    /// The expiry date is the last day of access, so a plan lapses only once
    /// the date is behind us. Roles are flipped in one statement — the
    /// `RETURNING` list is the authoritative set of users that this tick (and
    /// not a concurrent replica) downgraded, which keeps the notification
    /// emails exactly-once even though the whole tick is best-effort.
    async fn downgrade_expired(&self) -> Result<()> {
        let expired = sqlx::query!(
            r#"
            UPDATE users
               SET role = 'free',
                   plan_expiry_date = NULL,
                   plan_expiry_reminder_days = NULL
             WHERE plan_expiry_date < CURRENT_DATE
               AND role = 'professional'
         RETURNING id, first_name, email
            "#,
        )
        .fetch_all(self.db.as_ref())
        .await?;

        if expired.is_empty() {
            return Ok(());
        }

        for user in &expired {
            email::send_plan_expired_notice(&user.email, &user.first_name).await;
        }
        info!(downgraded = expired.len(), "expired admin-granted plans");
        Ok(())
    }

    /// Email everyone whose plan lapses within the widest reminder window.
    /// `plan_expiry_reminder_days` stores the smallest threshold already sent
    /// for the current date, so a tick that runs twice in a day — or a date
    /// that sits between two thresholds — never re-sends.
    async fn send_reminders(&self) -> Result<()> {
        let widest = REMINDER_THRESHOLDS[0];
        let candidates = sqlx::query_as!(
            ExpiringUser,
            r#"
            SELECT id,
                   first_name,
                   email,
                   plan_expiry_date AS "plan_expiry_date!",
                   plan_expiry_reminder_days
              FROM users
             WHERE plan_expiry_date IS NOT NULL
               AND plan_expiry_date >= CURRENT_DATE
               AND plan_expiry_date <= CURRENT_DATE + $1::INT
               AND role = 'professional'
            "#,
            widest as i32,
        )
        .fetch_all(self.db.as_ref())
        .await?;

        let today = Utc::now().date_naive();
        let mut sent = 0;
        for user in candidates {
            let days_remaining = (user.plan_expiry_date - today).num_days();
            let Some(threshold) = due_threshold(days_remaining, user.plan_expiry_reminder_days)
            else {
                continue;
            };

            // Record the reminder before sending it. A duplicate email is a
            // far worse outcome than a missed one, and the guard is scoped to
            // the expiry date the reminder was computed from so an admin
            // moving the date still re-arms the ladder.
            let marked = sqlx::query!(
                r#"
                UPDATE users
                   SET plan_expiry_reminder_days = $1
                 WHERE id = $2
                   AND plan_expiry_date = $3
                   AND (plan_expiry_reminder_days IS NULL OR plan_expiry_reminder_days > $1)
                "#,
                threshold as i32,
                user.id,
                user.plan_expiry_date,
            )
            .execute(self.db.as_ref())
            .await;

            match marked {
                Ok(result) if result.rows_affected() == 0 => continue,
                Ok(_) => {}
                Err(err) => {
                    warn!(user_id = user.id, error = %err, "could not record plan expiry reminder; skipping");
                    continue;
                }
            }

            email::send_plan_expiry_reminder(
                &user.email,
                &user.first_name,
                days_remaining,
                user.plan_expiry_date,
            )
            .await;
            sent += 1;
        }

        if sent > 0 {
            info!(sent, "sent plan expiry reminders");
        }
        Ok(())
    }
}

/// The reminder that is due today, if any: the smallest threshold the user has
/// reached that is still below whatever was last sent for this expiry date.
fn due_threshold(days_remaining: i64, already_sent: Option<i32>) -> Option<i64> {
    let reached = REMINDER_THRESHOLDS
        .into_iter()
        .filter(|threshold| days_remaining <= *threshold)
        .min()?;
    match already_sent {
        Some(sent) if i64::from(sent) <= reached => None,
        _ => Some(reached),
    }
}

#[cfg(test)]
mod tests {
    use super::due_threshold;

    #[test]
    fn picks_the_tightest_threshold_the_user_has_reached() {
        assert_eq!(due_threshold(30, None), Some(30));
        assert_eq!(due_threshold(8, None), Some(30));
        assert_eq!(due_threshold(7, None), Some(7));
        assert_eq!(due_threshold(1, None), Some(1));
        assert_eq!(due_threshold(0, None), Some(1));
    }

    #[test]
    fn stays_quiet_outside_the_widest_window() {
        assert_eq!(due_threshold(31, None), None);
        assert_eq!(due_threshold(365, None), None);
    }

    #[test]
    fn never_repeats_a_threshold_already_sent() {
        assert_eq!(due_threshold(30, Some(30)), None);
        assert_eq!(due_threshold(9, Some(30)), None);
        assert_eq!(due_threshold(7, Some(30)), Some(7));
        assert_eq!(due_threshold(2, Some(7)), None);
        assert_eq!(due_threshold(1, Some(7)), Some(1));
        assert_eq!(due_threshold(0, Some(1)), None);
    }

    /// A short grant skips the reminders it was never eligible for rather
    /// than firing them all on the first tick.
    #[test]
    fn short_grants_start_at_the_threshold_they_land_in() {
        assert_eq!(due_threshold(5, None), Some(7));
        assert_eq!(due_threshold(1, Some(7)), Some(1));
    }
}
