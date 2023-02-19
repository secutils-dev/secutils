CREATE TABLE IF NOT EXISTS users
(
    id              INTEGER PRIMARY KEY NOT NULL,
    email           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    handle          TEXT NOT NULL UNIQUE COLLATE NOCASE,
    credentials     BLOB NOT NULL,
    created         INTEGER NOT NULL,
    roles           TEXT,
    activation_code TEXT COLLATE NOCASE
) STRICT;

CREATE TABLE IF NOT EXISTS user_data
(
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    data_key    TEXT NOT NULL COLLATE NOCASE,
    data_value  BLOB NOT NULL,
    PRIMARY KEY (user_id, data_key)
) STRICT;

-- Table to store intermediate WebAuthn Relying Party session data during user registration and authentication.
CREATE TABLE IF NOT EXISTS user_webauthn_sessions
(
    email           TEXT PRIMARY KEY NOT NULL UNIQUE COLLATE NOCASE,
    session_value   BLOB NOT NULL,
    timestamp       INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS utils
(
    id          INTEGER PRIMARY KEY NOT NULL,
    handle      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    name        TEXT NOT NULL,
    keywords    TEXT NOT NULL,
    parent_id   INTEGER REFERENCES utils(id) ON DELETE CASCADE
) STRICT;

INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
   (1, 'home', 'Home', 'home start', NULL),
   (2, 'home__getting_started', 'Getting started', 'getting started', 1),
   (3, 'home__whats_new', 'What''s new', 'news updates what''s new', 1),
   (4, 'webhooks', 'Webhooks', 'webhooks hooks', NULL),
   (5, 'webhooks__responders', 'Responders', 'responders auto-responders respond http endpoint', 4),
   (6, 'certificates', 'Digital Certificates', 'digital certificates x509 X.509 ssl tls openssl public private key encryption pki', NULL),
   (7, 'certificates__self_signed_certificates', 'Self-signed certificates', 'digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki', 6),
   (8, 'web_security', 'Web Security', 'web security', NULL),
   (9, 'web_security__csp', 'CSP', 'csp content security policy', 8),
   (10, 'web_security__csp__policies', 'Policies', 'csp policies content security', 9),
   (11, 'web_scrapping', 'Web Scrapping', 'scrapping web puppeteer crawl spider', NULL),
   (12, 'web_scrapping__resources', 'Resources scrapper', 'web scrapping scrapper resources', 11);
