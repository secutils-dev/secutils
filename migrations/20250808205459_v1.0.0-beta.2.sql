-- Completed migration to the Retrack-based trackers, removing old tracker tables.
DROP TABLE IF EXISTS user_data_web_scraping_page_resources_trackers CASCADE;
DROP TABLE IF EXISTS user_data_web_scraping_trackers_history CASCADE;
DROP TABLE IF EXISTS user_data_web_scraping_trackers CASCADE;

-- Delete the old web scraping utilities.
DELETE FROM utils WHERE id IN (12, 13);
