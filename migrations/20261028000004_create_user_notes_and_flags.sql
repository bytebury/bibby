-- Private notes admins leave on an account ("refunded on 9/3", "emailed about
-- duplicate account"), shown on the admin user page. Only admins can read or
-- write them; the user never sees them.
CREATE TABLE user_notes
(
    id         SERIAL PRIMARY KEY,
    user_id    INTEGER     NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Kept nullable so a note outlives the admin who wrote it.
    author_id  INTEGER     REFERENCES users (id) ON DELETE SET NULL,
    body       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX user_notes_user_id_idx ON user_notes (user_id, created_at DESC);
CREATE INDEX user_notes_author_id_idx ON user_notes (author_id);

CREATE TRIGGER update_user_notes_modtime
    BEFORE UPDATE
    ON user_notes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Accounts an admin is keeping an eye on. Purely informational: a flagged
-- account signs in and works normally (unlike `users.locked`). Only admins
-- ever see flags.
CREATE TABLE user_flags
(
    user_id    INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- Nullable so the flag outlives the admin who set it.
    flagged_by INTEGER     REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX user_flags_flagged_by_idx ON user_flags (flagged_by);

CREATE TRIGGER update_user_flags_modtime
    BEFORE UPDATE
    ON user_flags
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
