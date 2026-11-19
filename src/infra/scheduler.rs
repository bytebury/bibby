//! The daily jobs, driven by `POST /_internal/cron/tick` (see
//! `infra/api/cron.rs`) so they run even when the web service has been asleep.

use crate::SharedState;
use crate::infra::db::DbPool;
use crate::prelude::*;

pub async fn run_tick(state: &SharedState) {
    info!("scheduler tick start");

    let Some(lease) = acquire_tick_lock(state.db.as_ref()).await else {
        return;
    };

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

    release_tick_lock(state.db.as_ref(), lease).await;
    info!("scheduler tick done");
}

/// Claim the daily tick via a Postgres-backed lease. Only the caller that
/// wins the upsert proceeds; the rest log and skip. The 30-minute
/// `held_until` is a crash safety net — a caller that dies mid-tick won't
/// wedge the system for long — and is long enough for a tick that waits on
/// slow mail sends. The `held_until` it wrote is returned as the holder's
/// token for `release_tick_lock`.
async fn acquire_tick_lock(db: &DbPool) -> Option<DateTime<Utc>> {
    let result = sqlx::query_scalar!(
        r#"
        INSERT INTO scheduler_locks (key, held_until)
        VALUES ('daily', NOW() + INTERVAL '30 minutes')
        ON CONFLICT (key) DO UPDATE
          SET held_until = EXCLUDED.held_until
          WHERE scheduler_locks.held_until < NOW()
        RETURNING held_until
        "#,
    )
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some(lease)) => Some(lease),
        Ok(None) => {
            info!("another tick already running; skipping");
            None
        }
        Err(error) => {
            error!(%error, "failed to acquire scheduler lock; skipping tick");
            None
        }
    }
}

/// Expire the lease on the happy path so the next cron ping can tick
/// immediately instead of waiting for the safety window to lapse. Only the
/// lease this tick took is released: if it ran past `held_until` and another
/// replica has since taken the lease, that one is left alone.
async fn release_tick_lock(db: &DbPool, lease: DateTime<Utc>) {
    if let Err(error) = sqlx::query!(
        "UPDATE scheduler_locks SET held_until = NOW() - INTERVAL '1 second' WHERE key = 'daily' AND held_until = $1",
        lease,
    )
    .execute(db)
    .await
    {
        error!(%error, "failed to release scheduler lock — will expire on its own");
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test]
    async fn only_one_caller_holds_the_daily_lease(db: PgPool) {
        let lease = acquire_tick_lock(&db)
            .await
            .expect("first caller gets the lease");
        assert!(
            acquire_tick_lock(&db).await.is_none(),
            "a second replica skips"
        );
        release_tick_lock(&db, lease).await;
        assert!(
            acquire_tick_lock(&db).await.is_some(),
            "released, so the next tick runs"
        );
    }

    #[sqlx::test]
    async fn a_tick_that_outlived_its_lease_leaves_the_new_holder_alone(db: PgPool) {
        let stale = acquire_tick_lock(&db).await.unwrap();
        // The lease lapses and another replica takes it.
        sqlx::query("UPDATE scheduler_locks SET held_until = NOW() - INTERVAL '1 second'")
            .execute(&db)
            .await
            .unwrap();
        acquire_tick_lock(&db).await.expect("new holder");
        release_tick_lock(&db, stale).await;
        assert!(
            acquire_tick_lock(&db).await.is_none(),
            "the new holder still has it"
        );
    }
}
