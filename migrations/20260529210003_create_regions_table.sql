CREATE TABLE regions
(
    id         SERIAL PRIMARY KEY,
    country_id INTEGER     NOT NULL REFERENCES countries (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (country_id, name)
);

CREATE INDEX idx_regions_country_id ON regions (country_id);

CREATE TRIGGER update_regions_modtime
    BEFORE UPDATE
    ON regions
    FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

ALTER TABLE users
    ADD COLUMN region_id INTEGER REFERENCES regions (id) ON DELETE SET NULL;
