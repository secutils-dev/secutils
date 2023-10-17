-- Register a new `Private keys` utility under `Digital certificates` and re-order certificate
-- utilities so that `Self-signed certificates` goes after `Private keys`.
UPDATE utils SET id = 11 WHERE id = 5;
INSERT INTO utils (id, handle, name, keywords, parent_id) VALUES
    (5, 'certificates__private_keys', 'Private keys', 'private keys openssl encryption pki rsa dsa ec ecdsa curve ed25519 pkcs8 pkcs12 pem', 4);

-- Create table to store private keys.
CREATE TABLE IF NOT EXISTS user_data_certificates_private_keys
(
    name            TEXT NOT NULL COLLATE NOCASE,
    alg             BLOB NOT NULL,
    pkcs8           BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY     (name, user_id)
) STRICT;
