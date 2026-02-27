-- Add secrets access column to page trackers. Stores postcard-encoded SecretsAccess.
-- Default \x00 is the postcard encoding of SecretsAccess::None.
ALTER TABLE user_data_web_scraping_page_trackers
ADD COLUMN secrets BYTEA NOT NULL DEFAULT '\x00';
