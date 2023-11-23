-- Create table to store responders.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    path            TEXT NOT NULL COLLATE NOCASE,
    method          BLOB NOT NULL,
    settings        BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id),
    UNIQUE          (path, method, user_id)
) STRICT;

-- Create table to store responders history.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders_history
(
    id              BLOB PRIMARY KEY,
    data            BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    responder_id    BLOB NOT NULL REFERENCES user_data_webhooks_responders(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
) STRICT;
