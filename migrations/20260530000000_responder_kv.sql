-- Per-responder key-value store backing the `secutils.kv.*` script primitive. Rows are scoped to a single responder and
-- cascade-deleted with it. The optional `expires_at` enables TTL semantics enforced both lazily (callers ignore expired
-- rows) and eagerly (the `WebhooksKvSweep` scheduler job).
CREATE TABLE user_data_webhooks_responders_kv (
    responder_id UUID NOT NULL REFERENCES user_data_webhooks_responders (id) ON DELETE CASCADE,
    -- `COLLATE "C"` pins key ordering to raw byte order so lexicographic prefix scans and `after` cursors are
    -- deterministic (ULID keys sort by time).
    key          TEXT COLLATE "C" NOT NULL,
    value        BYTEA NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL,
    expires_at   TIMESTAMPTZ NULL,
    PRIMARY KEY (responder_id, key)
);

-- Partial index keeps the periodic expiry sweep cheap: only rows that can ever expire are indexed, and the sweep is a
-- single ranged delete over this index.
CREATE INDEX user_data_webhooks_responders_kv_expires_idx
    ON user_data_webhooks_responders_kv (expires_at)
    WHERE expires_at IS NOT NULL;
