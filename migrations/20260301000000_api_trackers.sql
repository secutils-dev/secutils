-- Register the API trackers utility under the Web Scraping parent (id = 10).
INSERT INTO utils (id, handle, name, keywords, parent_id)
VALUES (12, 'web_scraping__api', 'API trackers',
        'web scraping api http rest json tracker track endpoint', 10);

-- Table for API tracker metadata. Actual tracker configuration and revisions
-- live in Retrack; this table links a Secutils user to a Retrack tracker.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_api_trackers
(
    id         UUID PRIMARY KEY NOT NULL,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    retrack_id UUID UNIQUE      NOT NULL,
    secrets    BYTEA            NOT NULL DEFAULT '\x00',
    created_at TIMESTAMPTZ      NOT NULL,
    updated_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);
