CREATE TABLE announcements
(
    id         SERIAL PRIMARY KEY,
    user_id    INTEGER     NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title      TEXT        NOT NULL,
    message    TEXT        NOT NULL,
    -- `active` is the published flag. Only one is rendered at a time (the
    -- most recent active row), so deactivating an old one is how you "retire"
    -- a banner without deleting it.
    active     BOOLEAN     NOT NULL DEFAULT true,
    severity   TEXT        NOT NULL DEFAULT 'info',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_announcements_user_id ON announcements (user_id);
CREATE INDEX idx_announcements_active_created ON announcements (active, created_at DESC);

CREATE TRIGGER update_announcements_modtime
    BEFORE UPDATE
    ON announcements
    FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
