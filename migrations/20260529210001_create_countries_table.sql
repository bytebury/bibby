CREATE TABLE countries
(
    id         SERIAL PRIMARY KEY,
    name       TEXT        NOT NULL UNIQUE,
    code       TEXT        NOT NULL UNIQUE,
    locked     BOOLEAN     NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_country_code ON countries (code);

CREATE TRIGGER update_countries_modtime
    BEFORE UPDATE
    ON countries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
