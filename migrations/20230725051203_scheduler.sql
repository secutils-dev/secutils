CREATE TABLE IF NOT EXISTS scheduler_jobs
(
    id              TEXT PRIMARY KEY NOT NULL COLLATE NOCASE,
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
    id              TEXT PRIMARY KEY NOT NULL COLLATE NOCASE,
    job_id          TEXT NOT NULL COLLATE NOCASE,
    extra           BLOB
) STRICT;

CREATE TABLE IF NOT EXISTS scheduler_notification_states
(
    id           TEXT NOT NULL COLLATE NOCASE REFERENCES scheduler_notifications(id) ON DELETE CASCADE,
    state        INTEGER NOT NULL,
    PRIMARY KEY (id, state)
) STRICT;
