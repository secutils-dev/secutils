-- Speed up home summary counts and "recent items" queries by user and last update time.
CREATE INDEX IF NOT EXISTS user_data_webhooks_responders_user_id_updated_at_idx
    ON user_data_webhooks_responders (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS user_data_certificates_certificate_templates_user_id_updated_at_idx
    ON user_data_certificates_certificate_templates (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS user_data_certificates_private_keys_user_id_updated_at_idx
    ON user_data_certificates_private_keys (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS user_data_web_security_csp_user_id_updated_at_idx
    ON user_data_web_security_csp (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS user_data_web_scraping_page_trackers_user_id_updated_at_idx
    ON user_data_web_scraping_page_trackers (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS user_data_web_scraping_api_trackers_user_id_updated_at_idx
    ON user_data_web_scraping_api_trackers (user_id, updated_at DESC);

-- Speed up notification scheduler pagination.
CREATE INDEX IF NOT EXISTS notifications_scheduled_at_id_idx
    ON notifications (scheduled_at, id);

-- Speed up responder history retrieval/cleanup by user and responder.
CREATE INDEX IF NOT EXISTS user_data_webhooks_responders_history_user_responder_created_at_idx
    ON user_data_webhooks_responders_history (user_id, responder_id, created_at);

-- Speed up utils tree loading order by parent utility.
CREATE INDEX IF NOT EXISTS utils_parent_id_id_idx
    ON utils (parent_id, id);
