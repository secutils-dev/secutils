-- Update `users` table to delete `roles` column (replaced by subscription).
ALTER TABLE users
    DROP COLUMN roles;

-- Upgrade scheduler table to add `time_offset_seconds` column (`tokio-cron-scheduler 0.10.0`).
ALTER TABLE scheduler_jobs
    ADD time_offset_seconds INTEGER;

-- Upgrade `user_data_webhooks_responders` table to add `enabled` column (defaults to `1`).
ALTER TABLE user_data_webhooks_responders
    ADD enabled INTEGER NOT NULL DEFAULT 1;

-- Create table to store user subscriptions.
CREATE TABLE IF NOT EXISTS user_subscriptions
(
    tier             INTEGER NOT NULL,
    started_at       INTEGER NOT NULL,
    ends_at          INTEGER,
    trial_started_at INTEGER,
    trial_ends_at    INTEGER,
    user_id          INTEGER NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    CHECK ((ends_at IS NULL OR (ends_at > started_at)) AND
           (trial_started_at IS NULL OR trial_ends_at IS NULL OR (trial_ends_at > trial_started_at)))
) STRICT;

-- Create subscription for all existing users (basic tier + 14 days of trial).
INSERT INTO user_subscriptions (user_id, tier, started_at, trial_started_at, trial_ends_at)
SELECT id, 10, created, unixepoch(), (unixepoch() + 14 * 24 * 60 * 60)
FROM users
WHERE true
ON CONFLICT(user_id) DO NOTHING;
