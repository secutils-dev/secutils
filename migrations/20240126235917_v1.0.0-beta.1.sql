-- Update `users` table to delete `roles` column (replaced by subscription).
ALTER TABLE users DROP COLUMN roles;

-- Create table to store user subscriptions.
CREATE TABLE IF NOT EXISTS user_subscriptions
(
    tier                INTEGER NOT NULL,
    started_at          INTEGER NOT NULL,
    ends_at             INTEGER,
    trial_started_at    INTEGER,
    trial_ends_at       INTEGER,
    user_id             INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    CHECK ((ends_at IS NULL OR (ends_at > started_at)) AND (trial_started_at IS NULL OR trial_ends_at IS NULL OR (trial_ends_at > trial_started_at)))
) STRICT;

-- Create subscription for all existing users (basic tier + 14 days of trial).
INSERT INTO user_subscriptions (user_id, tier, started_at, trial_started_at, trial_ends_at)
SELECT id, 10, created, unixepoch(), (unixepoch() + 14 * 24 * 60 * 60)
FROM   users
