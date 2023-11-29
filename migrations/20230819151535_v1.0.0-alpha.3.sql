CREATE TABLE IF NOT EXISTS scheduler_jobs
(
    id              BLOB PRIMARY KEY,
    last_updated    INTEGER,
    next_tick       INTEGER,
    last_tick       INTEGER,
    job_type        INTEGER NOT NULL,
    count           INTEGER,
    ran             INTEGER,
    stopped         INTEGER,
    schedule        TEXT COLLATE NOCASE,
    repeating       INTEGER,
    repeated_every  INTEGER,
    extra           BLOB
) STRICT;

CREATE TABLE IF NOT EXISTS scheduler_notifications
(
    id              BLOB PRIMARY KEY,
    job_id          BLOB NOT NULL,
    extra           BLOB
) STRICT;

CREATE TABLE IF NOT EXISTS scheduler_notification_states
(
    id           BLOB NOT NULL REFERENCES scheduler_notifications(id) ON DELETE CASCADE,
    state        INTEGER NOT NULL,
    PRIMARY KEY (id, state)
) STRICT;

CREATE TABLE IF NOT EXISTS notifications
(
    id              INTEGER PRIMARY KEY NOT NULL,
    destination     BLOB NOT NULL,
    content         BLOB NOT NULL,
    scheduled_at    INTEGER NOT NULL
) STRICT;

-- Table to store user public shares (content security policies, certificate templates etc.).
CREATE TABLE IF NOT EXISTS user_shares
(
    id              BLOB PRIMARY KEY,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resource        BLOB NOT NULL,
    created_at      INTEGER NOT NULL
) STRICT;
