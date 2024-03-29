-- Change "Web Scrapping" to "Web Scraping".
UPDATE utils
SET name = 'Web Scraping',
    handle = 'web_scraping'
WHERE
    id = 9;

-- Change "Resources scrapper" to "Resources trackers".
UPDATE utils
SET name = 'Resources trackers',
    keywords = 'web scraping crawl spider scraper scrape resources tracker track javascript css',
    handle = 'web_scraping__resources'
WHERE
    id = 10
