-- Every session token carries the account's `session_version`, and a token
-- whose version no longer matches is treated as signed out. Bumping it ends
-- every session at once: on a lock, an email change, or "Sign out
-- everywhere". Tokens minted before this column existed carry no version and
-- read as 0, so nobody is signed out by the deploy.
ALTER TABLE users
    ADD COLUMN session_version INTEGER NOT NULL DEFAULT 0;

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
       u.updated_at,
       u.session_version
FROM users u
         LEFT JOIN countries c ON u.country_id = c.id
         LEFT JOIN regions r ON u.region_id = r.id
         LEFT JOIN cities ci ON u.city_id = ci.id;
