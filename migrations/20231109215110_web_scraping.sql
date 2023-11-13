-- Register a new `Content trackers` utility under `Web Scraping` and re-order web scraping
-- utilities so that `Resources trackers` goes after `Content trackers`.
UPDATE utils SET id = 12 WHERE id = 10;
INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
    (10, 'web_scraping__content', 'Content trackers', 'web scraping crawl spider scraper scrape content tracker track', 9);

-- Create table to store web page trackers.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_trackers
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    url             TEXT NOT NULL,
    kind            BLOB NOT NULL,
    schedule        TEXT,
    job_id          BLOB UNIQUE,
    data            BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, kind, user_id)
) STRICT;

-- Create table to store web page trackers history.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_trackers_history
(
    id              BLOB PRIMARY KEY,
    data            BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    tracker_id      BLOB NOT NULL REFERENCES user_data_web_scraping_trackers(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (created_at, tracker_id)
) STRICT;
