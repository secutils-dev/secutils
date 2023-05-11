CREATE TABLE IF NOT EXISTS users
(
    id              INTEGER PRIMARY KEY NOT NULL,
    email           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    handle          TEXT NOT NULL UNIQUE COLLATE NOCASE,
    credentials     BLOB NOT NULL,
    created         INTEGER NOT NULL,
    roles           TEXT,
    activated       INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS user_data
(
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    namespace   TEXT NOT NULL COLLATE NOCASE,
    key         TEXT NOT NULL COLLATE NOCASE,
    value       BLOB NOT NULL,
    timestamp   INTEGER NOT NULL,
    PRIMARY KEY (user_id, namespace, key)
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
    keywords    TEXT,
    parent_id   INTEGER REFERENCES utils(id) ON DELETE CASCADE
) STRICT;

INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
   (1, 'home', 'Home', 'home start docs guides changes', NULL),
   (2, 'webhooks', 'Webhooks', NULL, NULL),
   (3, 'webhooks__responders', 'Responders', 'hooks webhooks responders auto-responders respond http endpoint', 2),
   (4, 'certificates', 'Digital Certificates', NULL, NULL),
   (5, 'certificates__self_signed_certificates', 'Self-signed certificates', 'digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki', 4),
   (6, 'web_security', 'Web Security', NULL, NULL),
   (7, 'web_security__csp', 'CSP', NULL, 6),
   (8, 'web_security__csp__policies', 'Policies', 'csp policies content web security', 7),
   (9, 'web_scrapping', 'Web Scrapping', NULL, NULL),
   (10, 'web_scrapping__resources', 'Resources scrapper', 'web scrapping crawl spider scrapper resources', 9);
