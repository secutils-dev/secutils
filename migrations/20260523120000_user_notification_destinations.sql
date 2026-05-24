-- Stores per-user notification delivery destinations (email today, more channels in future).
-- The verification fields drive the "claim then prove control" flow described in
-- docs/guides/platform/notification_email.mdx. The unsubscribe_token column powers the
-- public RFC 8058 one-click unsubscribe endpoint.
CREATE TABLE IF NOT EXISTS user_notification_destinations
(
    id                       UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    user_id                  UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Channel discriminator. v1 only accepts 'email'; the Rust enum is `#[non_exhaustive]`
    -- so future channels (slack, pagerduty, webhook) widen this value set without a schema change.
    kind                     TEXT             NOT NULL CHECK (kind IN ('email')),
    -- Channel-native handle. Lowercased for `email`. For future kinds it is e.g. the Slack user id.
    address                  TEXT             NOT NULL,
    -- Channel-specific configuration blob (empty for email).
    config                   JSONB            NOT NULL DEFAULT '{}'::jsonb,
    -- NULL until the recipient proves control of the destination.
    verified_at              TIMESTAMPTZ,
    -- Argon2id hash of the active verification code; cleared once the destination is verified or reset.
    verification_code_hash   TEXT,
    -- Cutoff after which `verification_code_hash` is no longer accepted.
    verification_expires_at  TIMESTAMPTZ,
    -- Timestamp of the last verification email send; used to enforce the 1-minute resend cooldown
    -- and the 5-per-hour rate limit.
    verification_sent_at     TIMESTAMPTZ,
    -- Failed code-entry attempts; lock at 5 and require a fresh code.
    verification_attempts    INT              NOT NULL DEFAULT 0,
    -- Random 32-byte token emitted in the `List-Unsubscribe` header for product notifications.
    unsubscribe_token        TEXT             NOT NULL UNIQUE,
    -- NULL while delivery is enabled; non-NULL after the recipient one-click unsubscribed.
    unsubscribed_at          TIMESTAMPTZ,
    created_at               TIMESTAMPTZ      NOT NULL,
    updated_at               TIMESTAMPTZ      NOT NULL,
    -- v1: at most one destination per (user, channel kind). Relaxes naturally to multi-destination
    -- later by widening the constraint or adding a per-row "primary" flag.
    UNIQUE (user_id, kind)
);
