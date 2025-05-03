-- Introduce a new trackers table to track web pages based on the Retrack service.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_page_trackers
(
    id          UUID PRIMARY KEY    NOT NULL,
    name        TEXT                NOT NULL COLLATE case_insensitive,
    retrack_id  UUID UNIQUE         NOT NULL,
    created_at  TIMESTAMPTZ         NOT NULL,
    updated_at  TIMESTAMPTZ         NOT NULL,
    user_id     UUID                NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);

-- Introduce a new trackers table to track web page resources based on the Retrack service.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_page_resources_trackers
(
    id          UUID PRIMARY KEY    NOT NULL,
    name        TEXT                NOT NULL COLLATE case_insensitive,
    retrack_id  UUID UNIQUE         NOT NULL,
    created_at  TIMESTAMPTZ         NOT NULL,
    updated_at  TIMESTAMPTZ         NOT NULL,
    user_id     UUID                NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);

-- Deprecate the old web scraping utilities and introduce new ones.
UPDATE utils SET id = 13, name = 'Resource trackers (deprecated)' WHERE id = 12;
UPDATE utils SET id = 12, name = 'Content trackers (deprecated)' WHERE id = 11;
INSERT INTO utils (id, handle, name, keywords, parent_id)
VALUES  (11, 'web_scraping__page', 'Page trackers',
         'web scraping crawl spider scraper scrape web page content tracker track', 10);
