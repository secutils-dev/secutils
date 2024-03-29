-- Register a new `Private keys` utility under `Digital certificates` and re-order certificate
-- utilities so that `Self-signed certificates` goes after `Private keys`.
UPDATE utils SET id = 11 WHERE id = 5;
INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
    (5, 'certificates__private_keys', 'Private keys', 'private keys openssl encryption pki rsa dsa ec ecdsa curve ed25519 pkcs8 pkcs12 pem', 4);

-- Register a new `Content trackers` utility under `Web Scraping` and re-order web scraping
-- utilities so that `Resources trackers` goes after `Content trackers`.
UPDATE utils SET id = 12 WHERE id = 10;
INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
    (10, 'web_scraping__content', 'Content trackers', 'web scraping crawl spider scraper scrape content tracker track', 9);

-- Change "Self-signed certificates" to "Certificate templates".
UPDATE utils
SET name = 'Certificate templates',
    handle = 'certificates__certificate_templates',
    keywords = 'digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki templates'
WHERE
    id = 11;

-- Create table to store private keys.
CREATE TABLE IF NOT EXISTS user_data_certificates_private_keys
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    alg             BLOB NOT NULL,
    pkcs8           BLOB NOT NULL,
    encrypted       INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id)
) STRICT;

-- Create table to store certificate templates.
CREATE TABLE IF NOT EXISTS user_data_certificates_certificate_templates
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    attributes      BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id)
) STRICT;

-- Create table to store web page trackers.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_trackers
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    url             TEXT NOT NULL,
    kind            BLOB NOT NULL,
    job_id          BLOB UNIQUE,
    job_config      BLOB,
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

-- Create table to store content security policies
CREATE TABLE IF NOT EXISTS user_data_web_security_csp
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    directives      BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id)
) STRICT;

-- Create table to store responders.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders
(
    id              BLOB PRIMARY KEY,
    name            TEXT NOT NULL COLLATE NOCASE,
    path            TEXT NOT NULL COLLATE NOCASE,
    method          BLOB NOT NULL,
    settings        BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE          (name, user_id),
    UNIQUE          (path, method, user_id)
) STRICT;

-- Create table to store responders history.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders_history
(
    id              BLOB PRIMARY KEY,
    data            BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    responder_id    BLOB NOT NULL REFERENCES user_data_webhooks_responders(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
) STRICT;
