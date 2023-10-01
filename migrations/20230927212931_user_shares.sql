-- Table to store user public shares (content security policies, certificate templates etc.).
CREATE TABLE IF NOT EXISTS user_shares
(
    id              TEXT PRIMARY KEY NOT NULL COLLATE NOCASE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resource        BLOB NOT NULL,
    created_at      INTEGER NOT NULL
) STRICT;
