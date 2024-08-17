-- Add `updated_at` column to the user certificate templates and pre-fill it with `created_at`.
ALTER TABLE user_data_certificates_certificate_templates ADD COLUMN updated_at TIMESTAMPTZ;
UPDATE user_data_certificates_certificate_templates SET updated_at = created_at;
ALTER TABLE user_data_certificates_certificate_templates ALTER COLUMN updated_at SET NOT NULL;

-- Add `updated_at` column to the user private keys and pre-fill it with `created_at`.
ALTER TABLE user_data_certificates_private_keys ADD COLUMN updated_at TIMESTAMPTZ;
UPDATE user_data_certificates_private_keys SET updated_at = created_at;
ALTER TABLE user_data_certificates_private_keys ALTER COLUMN updated_at SET NOT NULL;

-- Add `updated_at` column to the user trackers and pre-fill it with `created_at`.
ALTER TABLE user_data_web_scraping_trackers ADD COLUMN updated_at TIMESTAMPTZ;
UPDATE user_data_web_scraping_trackers SET updated_at = created_at;
ALTER TABLE user_data_web_scraping_trackers ALTER COLUMN updated_at SET NOT NULL;

-- Add `updated_at` column to the user content security policies and pre-fill it with `created_at`.
ALTER TABLE user_data_web_security_csp ADD COLUMN updated_at TIMESTAMPTZ;
UPDATE user_data_web_security_csp SET updated_at = created_at;
ALTER TABLE user_data_web_security_csp ALTER COLUMN updated_at SET NOT NULL;

-- Add `updated_at` column to the user responders and pre-fill it with `created_at`.
ALTER TABLE user_data_webhooks_responders ADD COLUMN updated_at TIMESTAMPTZ;
UPDATE user_data_webhooks_responders SET updated_at = created_at;
ALTER TABLE user_data_webhooks_responders ALTER COLUMN updated_at SET NOT NULL;
