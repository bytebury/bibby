-- Admin-curated groups of accounts that belong to the same person, so an
-- admin can see at a glance how many accounts someone has (e.g. spotting
-- spam sign-ups). A user is in at most one group; groups with fewer than two
-- members are deleted by the application when an account is unlinked.
CREATE TABLE account_groups
(
    id         SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER update_account_groups_modtime
    BEFORE UPDATE
    ON account_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE account_group_members
(
    user_id    INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    group_id   INTEGER     NOT NULL REFERENCES account_groups (id) ON DELETE CASCADE,
    -- The admin who linked this account in. Nullable so the link outlives them.
    linked_by  INTEGER     REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX account_group_members_group_id_idx ON account_group_members (group_id);
CREATE INDEX account_group_members_linked_by_idx ON account_group_members (linked_by);

CREATE TRIGGER update_account_group_members_modtime
    BEFORE UPDATE
    ON account_group_members
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
