CREATE TABLE blogs
(
    id         SERIAL PRIMARY KEY,
    user_id    INTEGER     NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title      TEXT        NOT NULL,
    content    TEXT        NOT NULL,
    image_url  TEXT        NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_blogs_user_id ON blogs (user_id);
CREATE INDEX idx_blogs_created_at ON blogs (created_at DESC);

CREATE TRIGGER update_blogs_modtime
    BEFORE UPDATE
    ON blogs
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
