-- One row per (user, UTC day) the user made an authenticated request.
-- Written by the per-request audit (`User::audit_and_find_by_email`) via an
-- ON CONFLICT DO NOTHING upsert, so repeat requests on the same day collapse
-- into a single row. Feeds the admin Site Analytics DAU chart and the
-- "days active" strip on the admin user page.
CREATE TABLE user_daily_activity
(
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day     DATE    NOT NULL,
    PRIMARY KEY (user_id, day)
);

CREATE INDEX user_daily_activity_day_idx ON user_daily_activity (day);
