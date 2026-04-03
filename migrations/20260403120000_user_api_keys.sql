CREATE TABLE IF NOT EXISTS user_api_keys
(
    id           UUID PRIMARY KEY NOT NULL,
    user_id      UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         TEXT             NOT NULL COLLATE case_insensitive,
    token_hash   BYTEA            NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ      NOT NULL,
    updated_at   TIMESTAMPTZ      NOT NULL,
    expires_at   TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    UNIQUE (name, user_id)
);
