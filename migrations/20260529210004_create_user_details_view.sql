-- `user_details` is the read path for the User model: it joins country and
-- region so handlers and templates can render flags and "<region>, <country>"
-- tooltips without a second query. COALESCE keeps country_code / country_name
-- non-null so the Rust struct binds them as `String` rather than `Option<String>`.
-- region_name stays nullable since many users only resolve to a country.
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
       r.name AS region_name,
       u.created_at,
       u.updated_at
FROM users u
         LEFT JOIN countries c ON u.country_id = c.id
         LEFT JOIN regions r ON u.region_id = r.id;
