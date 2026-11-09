//! The daily jobs, driven by `POST /_internal/cron/tick` (see
//! `infra/api/cron.rs`) so they run even when the web service has been asleep.

use crate::SharedState;
use crate::prelude::*;

pub async fn run_tick(state: &SharedState) {
    info!("scheduler tick start");

    if !acquire_tick_lock(state).await {
        return;
    }

    if let Err(error) = state
        .user_use_cases
        .process_plan_expirations
        .execute()
        .await
    {
        error!(%error, "plan expiration job failed");
    }

    if let Err(error) = state.user_use_cases.prune_sign_ins.execute().await {
        error!(%error, "sign-in pruning job failed");
    }

    release_tick_lock(state).await;
    info!("scheduler tick done");
}

/// Claim the daily tick via a Postgres-backed lease. Only the caller that
/// wins the upsert proceeds; the rest log and skip. The five-minute
/// `held_until` is a crash safety net — a caller that dies mid-tick won't
/// wedge the system for long. The happy-path release in `release_tick_lock`
/// expires the lease immediately, so back-to-back cron pings don't
/// shadow-skip each other.
async fn acquire_tick_lock(state: &SharedState) -> bool {
    let result = sqlx::query_scalar!(
        r#"
        INSERT INTO scheduler_locks (key, held_until)
        VALUES ('daily', NOW() + INTERVAL '5 minutes')
        ON CONFLICT (key) DO UPDATE
          SET held_until = EXCLUDED.held_until
          WHERE scheduler_locks.held_until < NOW()
        RETURNING key
        "#,
    )
    .fetch_optional(state.db.as_ref())
    .await;

    match result {
        Ok(Some(_)) => true,
        Ok(None) => {
            info!("another tick already running; skipping");
            false
        }
        Err(error) => {
            error!(%error, "failed to acquire scheduler lock; skipping tick");
            false
        }
    }
}

/// Expire the lease on the happy path so the next cron ping can tick
/// immediately instead of waiting for the safety window to lapse.
async fn release_tick_lock(state: &SharedState) {
    if let Err(error) = sqlx::query!(
        "UPDATE scheduler_locks SET held_until = NOW() - INTERVAL '1 second' WHERE key = 'daily'",
    )
    .execute(state.db.as_ref())
    .await
    {
        error!(%error, "failed to release scheduler lock — will expire on its own");
    }
}
