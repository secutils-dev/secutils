-- Change "Resources scrapper" to "Resources trackers".
UPDATE utils
SET name = 'Resources trackers',
    keywords = 'web scrapping crawl spider scrapper resources tracker track javascript css'
WHERE
    handle = 'web_scrapping__resources'
