-- Postgres-backed lease so only one replica runs the daily scheduler tick
-- (see `infra/scheduler.rs`).
CREATE TABLE scheduler_locks
(
    key        TEXT PRIMARY KEY,
    held_until TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER update_scheduler_locks_modtime
    BEFORE UPDATE
    ON scheduler_locks
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
