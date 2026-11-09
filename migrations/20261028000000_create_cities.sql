-- The city a sign-in geolocated to, below its region. Shown to admins on the
-- user page and in sign-in history.
CREATE TABLE cities
(
    id         SERIAL PRIMARY KEY,
    region_id  INTEGER     NOT NULL REFERENCES regions (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (region_id, name)
);

CREATE INDEX idx_cities_region_id ON cities (region_id);

CREATE TRIGGER update_cities_modtime
    BEFORE UPDATE
    ON cities
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

ALTER TABLE users
    ADD COLUMN city_id INTEGER REFERENCES cities (id) ON DELETE SET NULL;

CREATE INDEX idx_users_city_id ON users (city_id);
