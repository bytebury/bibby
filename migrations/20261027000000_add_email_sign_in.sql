-- Emailed one-time codes back both email sign-up (the first code verifies the
-- address) and passwordless sign-in. Keyed by email rather than user so a code
-- can be issued before anyone has proven they own the account. Only an HMAC of
-- the code is stored; the plaintext exists in the email alone.
CREATE TABLE one_time_passcodes
(
    id         SERIAL PRIMARY KEY,
    email      TEXT        NOT NULL,
    code_hash  BYTEA       NOT NULL,
    attempts   INTEGER     NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX one_time_passcodes_email_created_at_idx ON one_time_passcodes (email, created_at DESC);
CREATE INDEX one_time_passcodes_created_at_idx ON one_time_passcodes (created_at);

CREATE TRIGGER update_one_time_passcodes_modtime
    BEFORE UPDATE
    ON one_time_passcodes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- `auth_provider` records how an account was created: `email` accounts sign
-- in with an emailed code, `google` accounts may use either.
ALTER TABLE users
    ADD COLUMN auth_provider TEXT NOT NULL DEFAULT 'google'
        CHECK (auth_provider IN ('google', 'email'));

-- Unverified accounts can no longer hold a session. Every account that exists
-- today came through Google, so none of them should be locked out by that.
UPDATE users SET verified = TRUE WHERE verified = FALSE;

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
