-- Admin-granted Professional access can carry an optional end date.
-- `plan_expiry_date` is the last day the plan is active (inclusive); NULL means
-- the plan never expires. `plan_expiry_reminder_days` records the smallest
-- reminder threshold (30 / 7 / 1) already emailed for the current expiry date
-- so the daily scheduler tick never sends the same reminder twice.
ALTER TABLE users
    ADD COLUMN plan_expiry_date          DATE,
    ADD COLUMN plan_expiry_reminder_days INTEGER;

-- The expiration job scans by this column on every tick.
CREATE INDEX idx_users_plan_expiry_date ON users (plan_expiry_date)
    WHERE plan_expiry_date IS NOT NULL;

-- Matching other accounts on a user's last known IP (shared-IP locks and the
-- admin "same IP" suggestions) scans this column.
CREATE INDEX idx_users_last_known_ip ON users (last_known_ip);

DROP VIEW user_details;

CREATE VIEW user_details AS
SELECT u.id,
       u.full_name,
       u.first_name,
       u.last_name,
       u.email,
       u.image_url,
       u.role,
       u.verified,
       u.auth_provider,
       u.locked,
       u.plan_expiry_date,
       u.last_known_ip,
       u.last_seen_at,
       u.country_id,
       u.region_id,
       u.city_id,
       u.stripe_customer_id,
       COALESCE(c.code, 'unknown') AS country_code,
       COALESCE(c.name, 'Unknown') AS country_name,
       r.name  AS region_name,
       ci.name AS city_name,
       u.created_at,
       u.updated_at
FROM users u
         LEFT JOIN countries c ON u.country_id = c.id
         LEFT JOIN regions r ON u.region_id = r.id
         LEFT JOIN cities ci ON u.city_id = ci.id;
