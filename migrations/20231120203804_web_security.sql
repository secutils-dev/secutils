-- Create table to store content security policies
CREATE TABLE IF NOT EXISTS user_data_web_security_csp
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    directives      BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id)
) STRICT;
