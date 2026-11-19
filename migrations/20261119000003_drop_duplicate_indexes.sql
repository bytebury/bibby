-- Both columns are UNIQUE, and a UNIQUE constraint already has an index, so
-- these only cost writes.
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_country_code;
