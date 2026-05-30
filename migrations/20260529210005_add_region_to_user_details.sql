-- Extend `user_details` with the joined region name so flag tooltips can show
-- "<region>, <country>" without a second query. Region is optional: many
-- users will only have a country resolved (or nothing, in which case the
-- COALESCEd country defaults still apply).
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
       COALESCE(c.code, 'unknown') AS country_code,
       COALESCE(c.name, 'Unknown') AS country_name,
       r.name                       AS region_name,
       u.created_at,
       u.updated_at
FROM users u
         LEFT JOIN countries c ON u.country_id = c.id
         LEFT JOIN regions r ON u.region_id = r.id;
