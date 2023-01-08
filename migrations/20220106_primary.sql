CREATE TABLE IF NOT EXISTS users
(
    id              INTEGER PRIMARY KEY NOT NULL,
    email           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    handle          TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash   TEXT NOT NULL,
    created         INTEGER NOT NULL,
    roles           TEXT,
    activation_code TEXT COLLATE NOCASE
) STRICT;

CREATE TABLE IF NOT EXISTS user_data
(
    user_id     INTEGER NOT NULL,
    data_key    TEXT NOT NULL COLLATE NOCASE,
    data_value  BLOB NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id),
    PRIMARY KEY (user_id, data_key)
) STRICT;
