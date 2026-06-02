-- Adds support for "notify me when this responder is hit" notifications.
--
-- `notifications_enabled` is a denormalized flag (the throttle configuration itself lives in the
-- postcard-encoded `settings` blob, which is not SQL-queryable) so the notification scheduler job
-- can cheaply find responders that opted in. `notifications_last_at` tracks when the user was last
-- notified for the responder, and is used to throttle and coalesce subsequent notifications.
ALTER TABLE user_data_webhooks_responders
    ADD COLUMN IF NOT EXISTS notifications_enabled BOOL NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS notifications_last_at TIMESTAMPTZ;

-- Partial index so the scheduler job only scans responders that opted into notifications.
CREATE INDEX IF NOT EXISTS user_data_webhooks_responders_notifications_enabled_idx
    ON user_data_webhooks_responders (id)
    WHERE notifications_enabled;
