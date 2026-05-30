-- Add Stripe customer linkage so subscription state can survive checkout webhooks
-- and admins can look up a user's Stripe account from the manage-users view.
ALTER TABLE users
    ADD COLUMN stripe_customer_id TEXT;

CREATE INDEX idx_users_stripe_customer_id ON users (stripe_customer_id);

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
       u.locked,
       u.last_known_ip,
       u.last_seen_at,
       u.country_id,
       u.region_id,
       u.stripe_customer_id,
       COALESCE(c.code, 'unknown') AS country_code,
       COALESCE(c.name, 'Unknown') AS country_name,
       r.name                       AS region_name,
       u.created_at,
       u.updated_at
FROM users u
         LEFT JOIN countries c ON u.country_id = c.id
         LEFT JOIN regions r ON u.region_id = r.id;
