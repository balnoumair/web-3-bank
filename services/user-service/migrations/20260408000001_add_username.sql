-- Add username field to users.
--
-- username is nullable: users can register without one and set it later.
-- Uniqueness is case-insensitive: 'Alice' and 'alice' are the same username.
-- The partial index (WHERE username IS NOT NULL) ensures NULL values are not
-- treated as duplicates of each other.

ALTER TABLE users.users
  ADD COLUMN username TEXT;

CREATE UNIQUE INDEX users_username_lower_unique
  ON users.users (LOWER(username))
  WHERE username IS NOT NULL;
