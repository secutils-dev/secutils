CREATE COLLATION case_insensitive (provider = icu, locale = 'und-u-ks-level2', deterministic = false);

-- Table to store users.
CREATE TABLE IF NOT EXISTS users
(
    id          UUID PRIMARY KEY NOT NULL,
    email       TEXT             NOT NULL UNIQUE COLLATE case_insensitive,
    handle      TEXT             NOT NULL UNIQUE COLLATE case_insensitive,
    created_at  TIMESTAMPTZ      NOT NULL
);

-- Table to store user data (e.g., settings).
CREATE TABLE IF NOT EXISTS user_data
(
    user_id   UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    namespace TEXT        NOT NULL COLLATE case_insensitive,
    key       TEXT        NOT NULL COLLATE case_insensitive,
    value     BYTEA       NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, namespace, key)
);

-- Table to store private keys.
CREATE TABLE IF NOT EXISTS user_data_certificates_private_keys
(
    id         UUID PRIMARY KEY NOT NULL,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    alg        BYTEA            NOT NULL,
    pkcs8      BYTEA            NOT NULL,
    encrypted  BOOL             NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);

-- Table to store certificate templates.
CREATE TABLE IF NOT EXISTS user_data_certificates_certificate_templates
(
    id         UUID PRIMARY KEY NOT NULL,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    attributes BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);

-- Table to store web page trackers.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_trackers
(
    id         UUID PRIMARY KEY NOT NULL,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    url        TEXT             NOT NULL COLLATE case_insensitive,
    kind       BYTEA            NOT NULL,
    job_id     UUID UNIQUE,
    job_config BYTEA,
    data       BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, kind, user_id)
);

-- Table to store web page trackers history.
CREATE TABLE IF NOT EXISTS user_data_web_scraping_trackers_history
(
    id         UUID PRIMARY KEY NOT NULL,
    data       BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    tracker_id UUID             NOT NULL REFERENCES user_data_web_scraping_trackers (id) ON DELETE CASCADE,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (created_at, tracker_id)
);

-- Table to store content security policies
CREATE TABLE IF NOT EXISTS user_data_web_security_csp
(
    id         UUID PRIMARY KEY NOT NULL,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    directives BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id)
);

-- Table to store responders.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders
(
    id         UUID PRIMARY KEY NOT NULL,
    enabled    BOOL             NOT NULL DEFAULT TRUE,
    name       TEXT             NOT NULL COLLATE case_insensitive,
    path       TEXT             NOT NULL COLLATE case_insensitive,
    method     BYTEA            NOT NULL,
    settings   BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE (name, user_id),
    UNIQUE (path, method, user_id)
);

-- Table to store responders history.
CREATE TABLE IF NOT EXISTS user_data_webhooks_responders_history
(
    id           UUID PRIMARY KEY NOT NULL,
    data         BYTEA            NOT NULL,
    created_at   TIMESTAMPTZ      NOT NULL,
    responder_id UUID             NOT NULL REFERENCES user_data_webhooks_responders (id) ON DELETE CASCADE,
    user_id      UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE
);

-- Table to store user public shares (content security policies, certificate templates etc.).
CREATE TABLE IF NOT EXISTS user_shares
(
    id         UUID PRIMARY KEY NOT NULL,
    user_id    UUID             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    resource   BYTEA            NOT NULL,
    created_at TIMESTAMPTZ      NOT NULL
);

-- Table to store user subscriptions.
CREATE TABLE IF NOT EXISTS user_subscriptions
(
    tier             INTEGER     NOT NULL,
    started_at       TIMESTAMPTZ NOT NULL,
    ends_at          TIMESTAMPTZ,
    trial_started_at TIMESTAMPTZ,
    trial_ends_at    TIMESTAMPTZ,
    user_id          UUID UNIQUE NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    CHECK ((ends_at IS NULL OR (ends_at > started_at)) AND
           (trial_started_at IS NULL OR trial_ends_at IS NULL OR (trial_ends_at > trial_started_at)))
);

-- Table to store notifications.
CREATE TABLE IF NOT EXISTS notifications
(
    id           SERIAL PRIMARY KEY NOT NULL,
    destination  BYTEA              NOT NULL,
    content      BYTEA              NOT NULL,
    scheduled_at TIMESTAMPTZ        NOT NULL
);

-- Table to store all available utilities.
CREATE TABLE IF NOT EXISTS utils
(
    id        SERIAL PRIMARY KEY NOT NULL,
    handle    TEXT               NOT NULL UNIQUE COLLATE case_insensitive,
    name      TEXT               NOT NULL,
    keywords  TEXT,
    parent_id INTEGER REFERENCES utils (id) ON DELETE CASCADE
);

-- Table to store scheduler jobs.
CREATE TABLE IF NOT EXISTS scheduler_jobs
(
    id                  UUID PRIMARY KEY NOT NULL,
    last_updated        BIGINT,
    next_tick           BIGINT,
    last_tick           BIGINT,
    job_type            INTEGER          NOT NULL,
    count               INTEGER,
    ran                 BOOLEAN,
    stopped             BOOLEAN,
    schedule            TEXT,
    repeating           BOOLEAN,
    repeated_every      BIGINT,
    time_offset_seconds INTEGER,
    extra               BYTEA
);

-- Table to store scheduler job notifications.
CREATE TABLE IF NOT EXISTS scheduler_notifications
(
    id     UUID PRIMARY KEY NOT NULL,
    job_id UUID,
    extra  BYTEA
);

-- Table to store scheduler job notification states.
CREATE TABLE IF NOT EXISTS scheduler_notification_states
(
    id    UUID    NOT NULL REFERENCES scheduler_notifications (id) ON DELETE CASCADE,
    state INTEGER NOT NULL,
    PRIMARY KEY (id, state)
);

-- Insert utilities.
INSERT INTO utils (id, handle, name, keywords, parent_id)
VALUES (1, 'home', 'Home', 'home start docs guides changes', NULL),
       (2, 'webhooks', 'Webhooks', NULL, NULL),
       (3, 'webhooks__responders', 'Responders', 'hooks webhooks responders auto-responders respond http endpoint', 2),
       (4, 'certificates', 'Digital Certificates', NULL, NULL),
       (5, 'certificates__certificate_templates', 'Certificate templates',
        'digital certificates x509 X.509 ssl tls openssl public private key encryption self-signed pki templates', 4),
       (6, 'certificates__private_keys', 'Private keys',
        'private keys openssl encryption pki rsa dsa ec ecdsa curve ed25519 pkcs8 pkcs12 pem', 4),
       (7, 'web_security', 'Web Security', NULL, NULL),
       (8, 'web_security__csp', 'CSP', NULL, 7),
       (9, 'web_security__csp__policies', 'Policies', 'csp policies content web security', 8),
       (10, 'web_scraping', 'Web Scraping', NULL, NULL),
       (11, 'web_scraping__content', 'Content trackers',
        'web scraping crawl spider scraper scrape content tracker track', 10),
       (12, 'web_scraping__resources', 'Resources trackers',
        'web scraping crawl spider scraper scrape resources tracker track javascript css', 10);

-- Create subscription for all existing users (basic tier + 14 days of trial).
INSERT INTO user_subscriptions (user_id, tier, started_at, trial_started_at, trial_ends_at)
SELECT id, 10, current_timestamp, current_timestamp, (current_timestamp + INTERVAL '14 days')
FROM users
WHERE TRUE
ON CONFLICT(user_id) DO NOTHING;
