-- Home chain: set automatically on first observed deposit (via Treasury push).
-- Nullable until first deposit; sticky thereafter (not mutated by chain health).

ALTER TABLE users.users
  ADD COLUMN home_chain BIGINT;
