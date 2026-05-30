CREATE TABLE users
(
    id                 SERIAL PRIMARY KEY,
    full_name          TEXT        NOT NULL,
    first_name         TEXT        NOT NULL,
    last_name          TEXT,
    email              TEXT        NOT NULL UNIQUE,
    image_url          TEXT        NOT NULL DEFAULT '',
    role               TEXT        NOT NULL DEFAULT 'free',
    verified           BOOLEAN     NOT NULL DEFAULT false,
    locked             BOOLEAN     NOT NULL DEFAULT false,
    last_known_ip      TEXT        NOT NULL DEFAULT 'unknown',
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    country_id         INTEGER     REFERENCES countries (id) ON DELETE SET NULL,
    region_id          INTEGER     REFERENCES regions (id) ON DELETE SET NULL,
    stripe_customer_id TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users (email);
CREATE INDEX idx_users_stripe_customer_id ON users (stripe_customer_id);

CREATE TRIGGER update_users_modtime
    BEFORE UPDATE
    ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
