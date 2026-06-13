mod params;
mod types;

pub use self::{
    params::{
        ExportSelection, ExportTrackableSelection, UserDataExportInclude, UserDataExportParams,
    },
    types::{
        EXPORT_VERSION, ExportedPrivateKey, ExportedResponder, ExportedResponderRequest,
        ExportedTracker, UserDataExport, UserDataExportData,
    },
};

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        secrets::{SECRET_ENCRYPTION_MIN_PASSPHRASE_LENGTH, encrypt_secret_for_export},
        user_data::shared::DataFileSecret,
    },
};
use std::collections::HashMap;
use time::OffsetDateTime;

/// Generates a data export for the specified user.
pub async fn generate_export<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    params: &UserDataExportParams,
) -> anyhow::Result<UserDataExport> {
    let include = &params.include;

    // Export scripts.
    let scripts = match &include.scripts {
        None => vec![],
        Some(ExportSelection::All) => api.scripts(user).list_scripts(None).await?,
        Some(ExportSelection::Selected { ids }) => api.scripts(user).bulk_get_scripts(ids).await?,
    };

    // Export secrets, optionally with passphrase-encrypted values.
    let (secrets, secrets_encryption) = match &include.secrets {
        None => (vec![], None),
        Some(selection) => {
            let secrets_api = api.secrets(user);
            let all_secrets = match selection {
                ExportSelection::All => secrets_api.list_secrets().await?,
                ExportSelection::Selected { ids } => secrets_api.bulk_get_secrets(ids).await?,
            };

            if let Some(ref passphrase) = params.secrets_passphrase {
                if passphrase.len() < SECRET_ENCRYPTION_MIN_PASSPHRASE_LENGTH {
                    anyhow::bail!(
                        "Passphrase must be at least {SECRET_ENCRYPTION_MIN_PASSPHRASE_LENGTH} characters."
                    );
                }
                let meta = crate::users::secrets::SecretsEncryptionMeta::new();
                let decrypted: HashMap<_, _> = secrets_api.decrypt_all_secrets().await?;
                let mut exported = Vec::new();
                for secret in all_secrets {
                    let encrypted_value = decrypted.get(&secret.name).map(|plaintext: &String| {
                        encrypt_secret_for_export(plaintext.as_bytes(), passphrase, &meta)
                    });
                    let encrypted_value = match encrypted_value {
                        Some(Ok(v)) => Some(v),
                        Some(Err(e)) => {
                            tracing::warn!(
                                "Failed to encrypt secret '{}' for export: {e}",
                                secret.name
                            );
                            None
                        }
                        None => None,
                    };
                    exported.push(DataFileSecret {
                        id: secret.id,
                        name: secret.name,
                        encrypted_value,
                        tags: secret.tags,
                        created_at: secret.created_at,
                        updated_at: secret.updated_at,
                    });
                }
                (exported, Some(meta))
            } else {
                let secrets = all_secrets
                    .into_iter()
                    .map(DataFileSecret::from_secret)
                    .collect();
                (secrets, None)
            }
        }
    };

    // Export responders with optional history.
    let responders = match &include.responders {
        None => vec![],
        Some(selection) => {
            let webhooks = api.webhooks(user);
            let (responder_list, include_history) = match selection {
                ExportTrackableSelection::All { include_history } => {
                    (webhooks.get_responders().await?, *include_history)
                }
                ExportTrackableSelection::Selected {
                    ids,
                    include_history,
                } => (webhooks.bulk_get_responders(ids).await?, *include_history),
            };
            let mut all_requests = if include_history {
                webhooks
                    .bulk_get_responder_requests(
                        &responder_list.iter().map(|r| r.id).collect::<Vec<_>>(),
                    )
                    .await?
            } else {
                HashMap::new()
            };
            let mut exported = Vec::with_capacity(responder_list.len());
            for responder in responder_list {
                let history = all_requests
                    .remove(&responder.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(ExportedResponderRequest::from)
                    .collect();
                exported.push(ExportedResponder { responder, history });
            }
            exported
        }
    };

    // Export certificate templates.
    let certificate_templates = match &include.certificate_templates {
        None => vec![],
        Some(ExportSelection::All) => api.certificates(user).get_certificate_templates().await?,
        Some(ExportSelection::Selected { ids }) => {
            api.certificates(user)
                .bulk_get_certificate_templates(ids)
                .await?
        }
    };

    // Export private keys (with full pkcs8 data).
    let private_keys = match &include.private_keys {
        None => vec![],
        Some(ExportSelection::All) => api
            .db
            .certificates()
            .get_private_keys_for_export(user.id)
            .await?
            .into_iter()
            .map(ExportedPrivateKey::from)
            .collect(),
        Some(ExportSelection::Selected { ids }) => api
            .certificates(user)
            .bulk_get_private_keys_for_export(ids)
            .await?
            .into_iter()
            .map(ExportedPrivateKey::from)
            .collect(),
    };

    // Export CSPs.
    let content_security_policies = match &include.content_security_policies {
        None => vec![],
        Some(ExportSelection::All) => {
            api.web_security(user)
                .get_content_security_policies()
                .await?
        }
        Some(ExportSelection::Selected { ids }) => {
            api.web_security(user)
                .bulk_get_content_security_policies(ids)
                .await?
        }
    };

    // Export page trackers with optional history.
    let tracker_revisions = user
        .subscription
        .get_features(&api.config)
        .config
        .web_scraping
        .tracker_revisions;
    let page_trackers = match &include.page_trackers {
        None => vec![],
        Some(selection) => {
            let web_scraping = api.web_scraping(user);
            let (trackers, include_history) = match selection {
                ExportTrackableSelection::All { include_history } => {
                    (web_scraping.get_page_trackers().await?, *include_history)
                }
                ExportTrackableSelection::Selected {
                    ids,
                    include_history,
                } => (
                    web_scraping.bulk_get_page_trackers(ids).await?,
                    *include_history,
                ),
            };
            let mut all_histories = if include_history {
                let retrack_ids = trackers.iter().map(|t| t.retrack.id()).collect::<Vec<_>>();
                api.retrack()
                    .list_tracker_revisions_batch(&retrack_ids, tracker_revisions)
                    .await
                    .unwrap_or_default()
            } else {
                HashMap::new()
            };
            let mut exported = Vec::new();
            for tracker in trackers {
                let history = all_histories
                    .remove(&tracker.retrack.id())
                    .unwrap_or_default();
                if let Some(exported_tracker) = tracker.into_exported(history) {
                    exported.push(exported_tracker);
                }
            }
            exported
        }
    };

    // Export API trackers with optional history.
    let api_trackers = match &include.api_trackers {
        None => vec![],
        Some(selection) => {
            let web_scraping = api.web_scraping(user);
            let (trackers, include_history) = match selection {
                ExportTrackableSelection::All { include_history } => {
                    (web_scraping.get_api_trackers().await?, *include_history)
                }
                ExportTrackableSelection::Selected {
                    ids,
                    include_history,
                } => (
                    web_scraping.bulk_get_api_trackers(ids).await?,
                    *include_history,
                ),
            };
            let mut all_histories = if include_history {
                let retrack_ids = trackers.iter().map(|t| t.retrack.id()).collect::<Vec<_>>();
                api.retrack()
                    .list_tracker_revisions_batch(&retrack_ids, tracker_revisions)
                    .await
                    .unwrap_or_default()
            } else {
                HashMap::new()
            };
            let mut exported = Vec::new();
            for tracker in trackers {
                let history = all_histories
                    .remove(&tracker.retrack.id())
                    .unwrap_or_default();
                if let Some(exported_tracker) = tracker.into_exported(history) {
                    exported.push(exported_tracker);
                }
            }
            exported
        }
    };

    // Export settings.
    let settings = if include.settings {
        api.settings(user).get_settings().await?
    } else {
        None
    };

    // Export user tag definitions.
    let tags_api = api.tags(user);
    let tags = match &include.tags {
        None => vec![],
        Some(ExportSelection::All) => tags_api.list_tags().await?,
        Some(ExportSelection::Selected { ids }) => tags_api.bulk_get_tags(ids).await?,
    };

    Ok(UserDataExport {
        version: EXPORT_VERSION,
        exported_at: OffsetDateTime::now_utc(),
        secrets_encryption,
        data: UserDataExportData {
            tags,
            scripts,
            secrets,
            responders,
            certificate_templates,
            private_keys,
            content_security_policies,
            page_trackers,
            api_trackers,
            settings,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{params::UserDataExportInclude, *};
    use crate::{
        network::Network,
        retrack::tags::prepare_tags,
        tests::{
            MockResolver, RETRACK_RESOURCE_TAG, RETRACK_USER_TAG, mock_api_with_config,
            mock_api_with_network_and_config, mock_config, mock_retrack_api_tracker, mock_user,
        },
        users::{SecretCreateParams, SecretsAccess, scripts::ScriptCreateParams},
        utils::{
            certificates::{
                CertificateAttributes, PrivateKeyAlgorithm, PrivateKeySize,
                PrivateKeysCreateParams, SignatureAlgorithm, TemplatesCreateParams, Version,
            },
            web_scraping::{
                ApiTrackerConfig, ApiTrackerCreateParams, ApiTrackerTarget, PageTrackerConfig,
                PageTrackerCreateParams, PageTrackerTarget, TrackerKind,
            },
            web_security::{
                ContentSecurityPoliciesCreateParams, ContentSecurityPolicyContent,
                ContentSecurityPolicyDirective,
            },
            webhooks::{
                ResponderLocation, ResponderMethod, ResponderPathType, ResponderSettings,
                RespondersRequestCreateParams, tests::RespondersCreateParams,
            },
        },
    };
    use httpmock::MockServer;
    use reqwest::Client;
    use reqwest_middleware::ClientBuilder;
    use retrack_types::trackers::Page;
    use serde_json::json;
    use sqlx::PgPool;
    use std::borrow::Cow;
    use time::macros::datetime;

    fn empty_include() -> UserDataExportInclude {
        UserDataExportInclude {
            tags: None,
            scripts: None,
            secrets: None,
            responders: None,
            certificate_templates: None,
            private_keys: None,
            content_security_policies: None,
            page_trackers: None,
            api_trackers: None,
            settings: false,
        }
    }

    #[sqlx::test]
    async fn export_empty_selection(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: empty_include(),
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.version, EXPORT_VERSION);
        assert!(export.data.scripts.is_empty());
        assert!(export.data.secrets.is_empty());
        assert!(export.data.responders.is_empty());
        assert!(export.data.certificate_templates.is_empty());
        assert!(export.data.private_keys.is_empty());
        assert!(export.data.content_security_policies.is_empty());
        assert!(export.data.page_trackers.is_empty());
        assert!(export.data.api_trackers.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_scripts(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let script = api
            .scripts(&user)
            .create_script(ScriptCreateParams {
                name: "my_script".into(),
                script_type: "responder".into(),
                content: "console.log('hi')".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                scripts: Some(ExportSelection::Selected {
                    ids: vec![script.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.scripts.len(), 1);
        assert_eq!(export.data.scripts[0].name, "my_script");
        assert_eq!(export.data.scripts[0].content, "console.log('hi')");

        Ok(())
    }

    #[sqlx::test]
    async fn export_secrets_without_values(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let secret = api
            .secrets(&user)
            .create_secret(SecretCreateParams {
                name: "MY_KEY".into(),
                value: "secret-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                secrets: Some(ExportSelection::Selected {
                    ids: vec![secret.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.secrets.len(), 1);
        assert_eq!(export.data.secrets[0].name, "MY_KEY");
        // Verify the exported secret does NOT contain the encrypted value.
        let json = serde_json::to_value(&export.data.secrets[0])?;
        assert!(json.get("encryptedValue").is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn export_skips_nonexistent_ids(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                scripts: Some(ExportSelection::Selected {
                    ids: vec![uuid::Uuid::now_v7()],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert!(export.data.scripts.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_certificate_templates(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let templates = api.certificates(&user).get_certificate_templates().await?;
        assert!(templates.is_empty());

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: empty_include(),
        };

        let export = generate_export(&api, &user, &params).await?;
        assert!(export.data.certificate_templates.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_content_security_policies(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: empty_include(),
        };

        let export = generate_export(&api, &user, &params).await?;
        assert!(export.data.content_security_policies.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_private_keys_includes_pkcs8(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let key = api
            .certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "test-key".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;
        assert!(!key.pkcs8.is_empty(), "created key should have pkcs8 data");

        // Export with Selected.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                private_keys: Some(ExportSelection::Selected { ids: vec![key.id] }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.private_keys.len(), 1);
        assert_eq!(export.data.private_keys[0].name, "test-key");
        assert!(
            !export.data.private_keys[0].pkcs8.is_empty(),
            "exported private key must contain base64-encoded pkcs8 data"
        );
        // Verify the pkcs8 round-trips correctly.
        let decoded = openssl::base64::decode_block(&export.data.private_keys[0].pkcs8)?;
        assert_eq!(decoded, key.pkcs8);

        Ok(())
    }

    #[sqlx::test]
    async fn export_private_keys_all(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "key-1".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;
        api.certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "key-2".to_string(),
                alg: PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size2048,
                },
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;

        // Export All.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                private_keys: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.private_keys.len(), 2);
        for pk in &export.data.private_keys {
            assert!(
                !pk.pkcs8.is_empty(),
                "exported private key '{}' must contain pkcs8 data",
                pk.name
            );
        }

        Ok(())
    }

    #[sqlx::test]
    async fn export_scripts_all(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.scripts(&user)
            .create_script(ScriptCreateParams {
                name: "script_a".into(),
                script_type: "responder".into(),
                content: "a()".into(),
                tag_ids: vec![],
            })
            .await?;
        api.scripts(&user)
            .create_script(ScriptCreateParams {
                name: "script_b".into(),
                script_type: "responder".into(),
                content: "b()".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                scripts: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.scripts.len(), 2);

        Ok(())
    }

    #[sqlx::test]
    async fn export_secrets_all_without_passphrase(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.secrets(&user)
            .create_secret(SecretCreateParams {
                name: "KEY_A".into(),
                value: "value-a".into(),
                tag_ids: vec![],
            })
            .await?;
        api.secrets(&user)
            .create_secret(SecretCreateParams {
                name: "KEY_B".into(),
                value: "value-b".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                secrets: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.secrets.len(), 2);
        assert!(export.secrets_encryption.is_none());
        // Without passphrase, no encrypted values should be present.
        for secret in &export.data.secrets {
            assert!(secret.encrypted_value.is_none());
        }

        Ok(())
    }

    #[sqlx::test]
    async fn export_secrets_with_passphrase(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.secrets(&user)
            .create_secret(SecretCreateParams {
                name: "MY_KEY".into(),
                value: "secret-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: Some("my-long-passphrase-for-testing".to_string()),
            include: UserDataExportInclude {
                secrets: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.secrets.len(), 1);
        assert_eq!(export.data.secrets[0].name, "MY_KEY");
        // With passphrase, encrypted_value should be present.
        assert!(export.data.secrets[0].encrypted_value.is_some());
        // Secrets encryption metadata should be present.
        assert!(export.secrets_encryption.is_some());

        Ok(())
    }

    #[sqlx::test]
    async fn export_secrets_short_passphrase_fails(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.secrets(&user)
            .create_secret(SecretCreateParams {
                name: "MY_KEY".into(),
                value: "secret-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: Some("short".to_string()),
            include: UserDataExportInclude {
                secrets: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let result = generate_export(&api, &user, &params).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Passphrase must be at least")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn export_secrets_selected_subset(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let secret_a = api
            .secrets(&user)
            .create_secret(SecretCreateParams {
                name: "KEY_A".into(),
                value: "value-a".into(),
                tag_ids: vec![],
            })
            .await?;
        api.secrets(&user)
            .create_secret(SecretCreateParams {
                name: "KEY_B".into(),
                value: "value-b".into(),
                tag_ids: vec![],
            })
            .await?;

        // Export only KEY_A.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                secrets: Some(ExportSelection::Selected {
                    ids: vec![secret_a.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.secrets.len(), 1);
        assert_eq!(export.data.secrets[0].name, "KEY_A");

        Ok(())
    }

    #[sqlx::test]
    async fn export_scripts_selected_subset(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let script_a = api
            .scripts(&user)
            .create_script(ScriptCreateParams {
                name: "script_a".into(),
                script_type: "responder".into(),
                content: "a()".into(),
                tag_ids: vec![],
            })
            .await?;
        api.scripts(&user)
            .create_script(ScriptCreateParams {
                name: "script_b".into(),
                script_type: "responder".into(),
                content: "b()".into(),
                tag_ids: vec![],
            })
            .await?;

        // Export only script_a.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                scripts: Some(ExportSelection::Selected {
                    ids: vec![script_a.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.scripts.len(), 1);
        assert_eq!(export.data.scripts[0].name, "script_a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_certificate_templates_all_with_data(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.certificates(&user)
            .create_certificate_template(TemplatesCreateParams {
                template_name: "template-a".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn".to_string()),
                    country: None,
                    state_or_province: None,
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: datetime!(2020-01-01 00:00:00 UTC),
                    not_valid_after: datetime!(2030-01-01 00:00:00 UTC),
                    version: Version::One,
                    is_ca: false,
                    key_usage: None,
                    extended_key_usage: None,
                },
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                certificate_templates: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.certificate_templates.len(), 1);
        assert_eq!(export.data.certificate_templates[0].name, "template-a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_certificate_templates_selected(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let tmpl_a = api
            .certificates(&user)
            .create_certificate_template(TemplatesCreateParams {
                template_name: "tmpl-a".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn-a".to_string()),
                    country: None,
                    state_or_province: None,
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: datetime!(2020-01-01 00:00:00 UTC),
                    not_valid_after: datetime!(2030-01-01 00:00:00 UTC),
                    version: Version::One,
                    is_ca: false,
                    key_usage: None,
                    extended_key_usage: None,
                },
                tag_ids: vec![],
            })
            .await?;
        api.certificates(&user)
            .create_certificate_template(TemplatesCreateParams {
                template_name: "tmpl-b".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn-b".to_string()),
                    country: None,
                    state_or_province: None,
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: datetime!(2020-01-01 00:00:00 UTC),
                    not_valid_after: datetime!(2030-01-01 00:00:00 UTC),
                    version: Version::One,
                    is_ca: false,
                    key_usage: None,
                    extended_key_usage: None,
                },
                tag_ids: vec![],
            })
            .await?;

        // Export only tmpl-a.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                certificate_templates: Some(ExportSelection::Selected {
                    ids: vec![tmpl_a.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.certificate_templates.len(), 1);
        assert_eq!(export.data.certificate_templates[0].name, "tmpl-a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_content_security_policies_all_with_data(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.web_security(&user)
            .create_content_security_policy(ContentSecurityPoliciesCreateParams {
                name: "csp-a".to_string(),
                content: ContentSecurityPolicyContent::Directives(vec![
                    ContentSecurityPolicyDirective::UpgradeInsecureRequests,
                ]),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                content_security_policies: Some(ExportSelection::All),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.content_security_policies.len(), 1);
        assert_eq!(export.data.content_security_policies[0].name, "csp-a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_content_security_policies_selected(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let csp_a = api
            .web_security(&user)
            .create_content_security_policy(ContentSecurityPoliciesCreateParams {
                name: "csp-a".to_string(),
                content: ContentSecurityPolicyContent::Directives(vec![
                    ContentSecurityPolicyDirective::UpgradeInsecureRequests,
                ]),
                tag_ids: vec![],
            })
            .await?;
        api.web_security(&user)
            .create_content_security_policy(ContentSecurityPoliciesCreateParams {
                name: "csp-b".to_string(),
                content: ContentSecurityPolicyContent::Directives(vec![
                    ContentSecurityPolicyDirective::UpgradeInsecureRequests,
                ]),
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                content_security_policies: Some(ExportSelection::Selected {
                    ids: vec![csp_a.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.content_security_policies.len(), 1);
        assert_eq!(export.data.content_security_policies[0].name, "csp-a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_responders_all(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "resp-a".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/a".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                responders: Some(ExportTrackableSelection::All {
                    include_history: false,
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.responders.len(), 1);
        assert_eq!(export.data.responders[0].responder.name, "resp-a");
        assert!(export.data.responders[0].history.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_responders_selected(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let resp_a = api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "resp-a".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/a".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Get,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tag_ids: vec![],
            })
            .await?;
        api.webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "resp-b".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/b".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Post,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 3,
                    status_code: 201,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tag_ids: vec![],
            })
            .await?;

        // Export only resp-a.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                responders: Some(ExportTrackableSelection::Selected {
                    ids: vec![resp_a.id],
                    include_history: false,
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.responders.len(), 1);
        assert_eq!(export.data.responders[0].responder.name, "resp-a");

        Ok(())
    }

    #[sqlx::test]
    async fn export_responders_with_history(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let responder = api
            .webhooks(&user)
            .create_responder(RespondersCreateParams {
                name: "resp-history".to_string(),
                location: ResponderLocation {
                    path_type: ResponderPathType::Exact,
                    path: "/hist".to_string(),
                    subdomain_prefix: None,
                },
                method: ResponderMethod::Any,
                enabled: true,
                settings: ResponderSettings {
                    requests_to_track: 10,
                    status_code: 200,
                    body: None,
                    headers: None,
                    script: None,
                    secrets: SecretsAccess::None,
                    notifications: None,
                },
                tag_ids: vec![],
            })
            .await?;

        // Create a request for this responder.
        api.webhooks(&user)
            .create_responder_request(
                responder.id,
                RespondersRequestCreateParams {
                    client_address: None,
                    method: Cow::Borrowed("GET"),
                    headers: None,
                    url: Cow::Borrowed("/hist"),
                    body: None,
                    duration_ms: None,
                    response_status_code: None,
                    response_headers: None,
                    response_body: None,
                },
            )
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                responders: Some(ExportTrackableSelection::All {
                    include_history: true,
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.responders.len(), 1);
        assert_eq!(export.data.responders[0].responder.name, "resp-history");
        assert_eq!(export.data.responders[0].history.len(), 1);
        assert_eq!(export.data.responders[0].history[0].url, "/hist");

        Ok(())
    }

    #[sqlx::test]
    async fn export_multiple_entity_types(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let script = api
            .scripts(&user)
            .create_script(ScriptCreateParams {
                name: "export-script".into(),
                script_type: "responder".into(),
                content: "test()".into(),
                tag_ids: vec![],
            })
            .await?;
        let key = api
            .certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "export-key".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                scripts: Some(ExportSelection::Selected {
                    ids: vec![script.id],
                }),
                private_keys: Some(ExportSelection::Selected { ids: vec![key.id] }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.scripts.len(), 1);
        assert_eq!(export.data.scripts[0].name, "export-script");
        assert_eq!(export.data.private_keys.len(), 1);
        assert_eq!(export.data.private_keys[0].name, "export-key");
        assert!(!export.data.private_keys[0].pkcs8.is_empty());
        // Other types should be empty.
        assert!(export.data.secrets.is_empty());
        assert!(export.data.responders.is_empty());
        assert!(export.data.certificate_templates.is_empty());
        assert!(export.data.content_security_policies.is_empty());
        assert!(export.data.page_trackers.is_empty());
        assert!(export.data.api_trackers.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_private_keys_selected_subset(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let key_a = api
            .certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "key-a".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;
        api.certificates(&user)
            .create_private_key(PrivateKeysCreateParams {
                key_name: "key-b".to_string(),
                alg: PrivateKeyAlgorithm::Ed25519,
                passphrase: None,
                tag_ids: vec![],
            })
            .await?;

        // Export only key-a.
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                private_keys: Some(ExportSelection::Selected {
                    ids: vec![key_a.id],
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert_eq!(export.data.private_keys.len(), 1);
        assert_eq!(export.data.private_keys[0].name, "key-a");
        assert!(!export.data.private_keys[0].pkcs8.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_skips_nonexistent_ids_for_all_types(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let fake_id = uuid::Uuid::now_v7();
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                tags: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                scripts: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                secrets: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                certificate_templates: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                private_keys: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                content_security_policies: Some(ExportSelection::Selected { ids: vec![fake_id] }),
                responders: Some(ExportTrackableSelection::Selected {
                    ids: vec![fake_id],
                    include_history: false,
                }),
                page_trackers: Some(ExportTrackableSelection::Selected {
                    ids: vec![fake_id],
                    include_history: false,
                }),
                api_trackers: Some(ExportTrackableSelection::Selected {
                    ids: vec![fake_id],
                    include_history: false,
                }),
                settings: false,
            },
        };

        let export = generate_export(&api, &user, &params).await?;
        assert!(export.data.scripts.is_empty());
        assert!(export.data.secrets.is_empty());
        assert!(export.data.certificate_templates.is_empty());
        assert!(export.data.private_keys.is_empty());
        assert!(export.data.content_security_policies.is_empty());
        assert!(export.data.responders.is_empty());
        assert!(export.data.page_trackers.is_empty());
        assert!(export.data.api_trackers.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn export_version_and_timestamp(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let before = OffsetDateTime::now_utc();
        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: empty_include(),
        };
        let export = generate_export(&api, &user, &params).await?;
        let after = OffsetDateTime::now_utc();

        assert_eq!(export.version, EXPORT_VERSION);
        assert!(export.exported_at >= before);
        assert!(export.exported_at <= after);

        Ok(())
    }

    #[sqlx::test]
    async fn export_page_trackers_history_uses_subscription_revisions_limit(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        // Set subscription tracker_revisions to 3 - explicitly different from DEFAULT_REVISIONS_BATCH_SIZE (10).
        let mut config = mock_config()?;
        config.subscriptions.ultimate.web_scraping.tracker_revisions = 3;

        let retrack_server = MockServer::start();
        config.retrack.host = url::Url::parse(&retrack_server.base_url())?;

        let mock_user = mock_user()?;
        let network = Network::new(
            MockResolver::new(),
            lettre::transport::stub::AsyncStubTransport::new_ok(),
            ClientBuilder::new(Client::builder().build()?).build(),
        );
        let api = mock_api_with_network_and_config(pool, network, config).await?;
        api.db.upsert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        // Mock Retrack tracker creation.
        let retrack_tracker = crate::retrack::tests::mock_retrack_tracker()?;
        let retrack_create_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "tracker-with-history".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { return ''; }".to_string(),
                    accept_invalid_certificates: false,
                    engine: None,
                },
                notifications: false,
                secrets: Default::default(),
                tag_ids: vec![],
            })
            .await?;
        retrack_create_mock.assert();

        // Mock GET /api/trackers for the `All` export path.
        let tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", TrackerKind::Page),
        ])
        .into_iter()
        .collect::<Vec<_>>();
        let retrack_list_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Page::new(vec![retrack_tracker.clone()], 1));
        });

        // Mock POST /api/trackers/revisions - assert size equals subscription limit (3, not DEFAULT=10).
        let revision = retrack_types::trackers::TrackerDataRevision {
            id: uuid::uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: retrack_tracker.id,
            data: retrack_types::trackers::TrackerDataValue::new(json!("rev1")),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };
        let retrack_revisions_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers/revisions")
                .json_body(json!({
                    "trackerIds": [retrack_tracker.id],
                    "size": 3
                }));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&HashMap::from([(retrack_tracker.id, vec![revision])]));
        });

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                page_trackers: Some(ExportTrackableSelection::All {
                    include_history: true,
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &mock_user, &params).await?;
        retrack_list_mock.assert();
        retrack_revisions_mock.assert();

        assert_eq!(export.data.page_trackers.len(), 1);
        assert_eq!(export.data.page_trackers[0].history.len(), 1);
        assert_eq!(
            export.data.page_trackers[0].history[0].data.original(),
            &json!("rev1")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn export_api_trackers_history_uses_subscription_revisions_limit(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        // Set subscription tracker_revisions to 3 - explicitly different from DEFAULT_REVISIONS_BATCH_SIZE (10).
        let mut config = mock_config()?;
        config.subscriptions.ultimate.web_scraping.tracker_revisions = 3;

        let retrack_server = MockServer::start();
        config.retrack.host = url::Url::parse(&retrack_server.base_url())?;

        let mock_user = mock_user()?;
        let network = Network::new(
            MockResolver::new(),
            lettre::transport::stub::AsyncStubTransport::new_ok(),
            ClientBuilder::new(Client::builder().build()?).build(),
        );
        let api = mock_api_with_network_and_config(pool, network, config).await?;
        api.db.upsert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        // Mock Retrack tracker creation.
        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api-tracker-with-history".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: false,
                secrets: Default::default(),
                tag_ids: vec![],
            })
            .await?;
        retrack_create_mock.assert();

        // Mock GET /api/trackers for the `All` export path.
        let tags = prepare_tags(&[
            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
            format!("{RETRACK_RESOURCE_TAG}:{}", TrackerKind::Api),
        ])
        .into_iter()
        .collect::<Vec<_>>();
        let retrack_list_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Page::new(vec![retrack_tracker.clone()], 1));
        });

        // Mock POST /api/trackers/revisions - assert size equals subscription limit (3, not DEFAULT=10).
        let revision = retrack_types::trackers::TrackerDataRevision {
            id: uuid::uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: retrack_tracker.id,
            data: retrack_types::trackers::TrackerDataValue::new(json!("api-rev1")),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };
        let retrack_revisions_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers/revisions")
                .json_body(json!({
                    "trackerIds": [retrack_tracker.id],
                    "size": 3
                }));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&HashMap::from([(retrack_tracker.id, vec![revision])]));
        });

        let params = UserDataExportParams {
            secrets_passphrase: None,
            include: UserDataExportInclude {
                api_trackers: Some(ExportTrackableSelection::All {
                    include_history: true,
                }),
                ..empty_include()
            },
        };

        let export = generate_export(&api, &mock_user, &params).await?;
        retrack_list_mock.assert();
        retrack_revisions_mock.assert();

        assert_eq!(export.data.api_trackers.len(), 1);
        assert_eq!(export.data.api_trackers[0].history.len(), 1);
        assert_eq!(
            export.data.api_trackers[0].history[0].data.original(),
            &json!("api-rev1")
        );

        Ok(())
    }
}
