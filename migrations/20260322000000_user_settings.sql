-- Migrate user settings from the generic user_data table to a dedicated user_settings table.
CREATE TABLE user_settings (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE PRIMARY KEY,
    value      BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

INSERT INTO user_settings (user_id, value, created_at, updated_at)
SELECT user_id, value, timestamp, timestamp FROM user_data
WHERE namespace = 'userSettings' AND key = '';

DROP TABLE user_data;
