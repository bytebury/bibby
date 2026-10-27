-- One row per successful sign-in (Google or emailed code), with the IP and
-- where it geolocated to at the time. Shown to admins on the user page and
-- used to suggest accounts that may belong to the same person.
--
-- Rows older than 30 days are pruned by the daily tick, except each user's
-- most recent sign-in, which is kept no matter how old it is.
CREATE TABLE user_sign_ins
(
    id         SERIAL PRIMARY KEY,
    user_id    INTEGER     NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    ip         TEXT        NOT NULL,
    country_id INTEGER     REFERENCES countries (id) ON DELETE SET NULL,
    region_id  INTEGER     REFERENCES regions (id) ON DELETE SET NULL,
    city_id    INTEGER     REFERENCES cities (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX user_sign_ins_user_id_idx ON user_sign_ins (user_id, created_at DESC);
CREATE INDEX user_sign_ins_ip_idx ON user_sign_ins (ip);
CREATE INDEX user_sign_ins_created_at_idx ON user_sign_ins (created_at);
CREATE INDEX user_sign_ins_country_id_idx ON user_sign_ins (country_id);
CREATE INDEX user_sign_ins_region_id_idx ON user_sign_ins (region_id);
CREATE INDEX user_sign_ins_city_id_idx ON user_sign_ins (city_id);

CREATE TRIGGER update_user_sign_ins_modtime
    BEFORE UPDATE
    ON user_sign_ins
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
