-- Managed user tags with color support, plus junction tables linking tags to all entity types.
CREATE TABLE user_tags (
    id         UUID PRIMARY KEY NOT NULL,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL COLLATE case_insensitive,
    color      TEXT NOT NULL DEFAULT 'default',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (user_id, name)
);

CREATE TABLE user_data_webhooks_responders_tags (
    responder_id UUID NOT NULL REFERENCES user_data_webhooks_responders(id) ON DELETE CASCADE,
    tag_id       UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (responder_id, tag_id)
);

CREATE TABLE user_data_certificates_certificate_templates_tags (
    template_id UUID NOT NULL REFERENCES user_data_certificates_certificate_templates(id) ON DELETE CASCADE,
    tag_id      UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (template_id, tag_id)
);

CREATE TABLE user_data_certificates_private_keys_tags (
    key_id UUID NOT NULL REFERENCES user_data_certificates_private_keys(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (key_id, tag_id)
);

CREATE TABLE user_data_web_security_csp_tags (
    csp_id UUID NOT NULL REFERENCES user_data_web_security_csp(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (csp_id, tag_id)
);

CREATE TABLE user_data_web_scraping_page_trackers_tags (
    tracker_id UUID NOT NULL REFERENCES user_data_web_scraping_page_trackers(id) ON DELETE CASCADE,
    tag_id     UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (tracker_id, tag_id)
);

CREATE TABLE user_data_web_scraping_api_trackers_tags (
    tracker_id UUID NOT NULL REFERENCES user_data_web_scraping_api_trackers(id) ON DELETE CASCADE,
    tag_id     UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (tracker_id, tag_id)
);

CREATE TABLE user_data_secrets_tags (
    secret_id UUID NOT NULL REFERENCES user_data_secrets(id) ON DELETE CASCADE,
    tag_id    UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (secret_id, tag_id)
);

CREATE TABLE user_data_scripts_tags (
    script_id UUID NOT NULL REFERENCES user_data_scripts(id) ON DELETE CASCADE,
    tag_id    UUID NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (script_id, tag_id)
);
