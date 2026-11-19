-- Admin-controlled switches, one row per feature the code knows about (see
-- `Feature` in src/domain/feature_toggle.rs). A toggle is on or off and can
-- carry a value, e.g. the Google Analytics measurement ID.
CREATE TABLE feature_toggles (
    id         SERIAL PRIMARY KEY,
    key        TEXT        NOT NULL UNIQUE,
    enabled    BOOLEAN     NOT NULL DEFAULT FALSE,
    value      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER update_feature_toggles_modtime
    BEFORE UPDATE
    ON feature_toggles
    FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Off until an admin turns it on and enters a measurement ID.
INSERT INTO feature_toggles (key) VALUES ('google_analytics');
