use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    users::{
        User, UserDataExportParams,
        user_data::{
            export::{ExportSelection, ExportTrackableSelection, UserDataExportInclude},
            generate_export,
            import::{
                ConflictResolution, ImportAction, ImportEntitySelection, ImportMode,
                ImportSelections, UserDataImportFile, UserDataImportFileData, UserDataImportParams,
                UserDataImportResult, execute_import,
            },
        },
    },
};
use anyhow::Context;
use hex::ToHex;

/// Length of the ephemeral passphrase used to round-trip secret values through the
/// export/import pipeline during a user clone. The passphrase never leaves this process.
const EPHEMERAL_SECRETS_PASSPHRASE_LENGTH: usize = 48;

/// Summary of a clone operation: counts of each entity type that was cloned.
///
/// Re-exposes [`UserDataImportResult`] under a clearer name so the public clone endpoint's
/// response shape reads naturally to operators.
pub type UserDataCloneSummary = UserDataImportResult;

/// Copies every entity owned by `source` (tags, scripts, secrets, responders, certificate
/// templates, private keys, content security policies, page/API trackers, settings) into
/// `destination`, regenerating IDs as needed. Reuses the standard export/import pipeline so
/// that the exact same code paths exercised by the public `/api/user/data/_export` and
/// `/api/user/data/_import` endpoints run here too.
///
/// Secrets are round-tripped through the export-encryption layer using an ephemeral
/// passphrase generated and held only for the duration of this call. The passphrase is
/// dropped on return and never leaves the process.
pub async fn clone_user_data<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    source: &User,
    destination: &User,
    include_history: bool,
) -> anyhow::Result<UserDataCloneSummary>
where
    ET::Error: EmailTransportError,
{
    let secrets_passphrase = ephemeral_passphrase();

    let export_params = UserDataExportParams {
        include: UserDataExportInclude {
            tags: Some(ExportSelection::All),
            scripts: Some(ExportSelection::All),
            secrets: Some(ExportSelection::All),
            responders: Some(ExportTrackableSelection::All { include_history }),
            certificate_templates: Some(ExportSelection::All),
            private_keys: Some(ExportSelection::All),
            content_security_policies: Some(ExportSelection::All),
            page_trackers: Some(ExportTrackableSelection::All { include_history }),
            api_trackers: Some(ExportTrackableSelection::All { include_history }),
            settings: true,
        },
        secrets_passphrase: Some(secrets_passphrase.clone()),
    };

    let export = generate_export(api, source, &export_params)
        .await
        .with_context(|| "Failed to generate source user data export for clone.")?;

    // Round-trip the in-memory export through JSON to produce the structurally compatible
    // import file. Both shapes share the same field names and `secretsEncryption` envelope,
    // so this is a straight serde transcode rather than a per-field copy.
    let import_file: UserDataImportFile = serde_json::from_value(serde_json::to_value(&export)?)
        .with_context(|| "Failed to convert export into import file shape for clone.")?;

    let import_params = UserDataImportParams {
        selections: build_all_selections(&import_file.data),
        data: import_file,
        mode: ImportMode::Merge,
        secrets_passphrase: Some(secrets_passphrase),
        apply_deletions: None,
    };

    execute_import(api, destination, import_params)
        .await
        .with_context(|| "Failed to import data into destination user during clone.")
}

/// Builds an `ImportSelections` that imports every entity present in `data`. The destination
/// user is brand-new, so there are no conflicts to resolve.
fn build_all_selections(data: &UserDataImportFileData) -> ImportSelections {
    fn import_all<I, F>(items: I, get_id: F) -> Vec<ImportEntitySelection>
    where
        I: IntoIterator,
        F: Fn(&I::Item) -> uuid::Uuid,
    {
        items
            .into_iter()
            .map(|item| ImportEntitySelection {
                source_id: get_id(&item),
                action: ImportAction::Import,
                // Destination is empty so this is unused, but Skip would be a footgun if the
                // destination ever turns out to have stale state — leave at None.
                conflict_resolution: None::<ConflictResolution>,
            })
            .collect()
    }

    ImportSelections {
        tags: import_all(data.tags.iter(), |t| t.id),
        scripts: import_all(data.scripts.iter(), |s| s.id),
        secrets: import_all(data.secrets.iter(), |s| s.id),
        responders: import_all(data.responders.iter(), |r| r.responder.id),
        certificate_templates: import_all(data.certificate_templates.iter(), |t| t.id),
        private_keys: import_all(data.private_keys.iter(), |k| k.id),
        content_security_policies: import_all(data.content_security_policies.iter(), |c| c.id),
        page_trackers: import_all(data.page_trackers.iter(), |t| t.id),
        api_trackers: import_all(data.api_trackers.iter(), |t| t.id),
        import_settings: true,
    }
}

/// Generates a cryptographically random ASCII passphrase used exclusively to feed the
/// export → import pipeline during [`clone_user_data`].
fn ephemeral_passphrase() -> String {
    let mut bytes = [0u8; EPHEMERAL_SECRETS_PASSPHRASE_LENGTH];
    // A `getrandom` failure means the OS entropy source is unavailable, which is fatal
    // anyway; panicking is consistent with `SecurityApiExt::generate_user_handle`.
    getrandom::fill(&mut bytes).expect("Failed to draw random bytes for ephemeral passphrase.");
    bytes.encode_hex::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user_with_id},
        users::{SecretCreateParams, scripts::ScriptCreateParams},
        utils::web_security::{
            ContentSecurityPoliciesCreateParams, ContentSecurityPolicyContent,
            ContentSecurityPolicyDirective,
        },
    };
    use httpmock::MockServer;
    use retrack_types::trackers::{Page, Tracker};
    use sqlx::PgPool;
    use uuid::uuid;

    #[test]
    fn ephemeral_passphrase_is_long_enough() {
        // Must exceed `SECRET_ENCRYPTION_MIN_PASSPHRASE_LENGTH` (8 bytes).
        let pass = ephemeral_passphrase();
        assert!(
            pass.len() >= EPHEMERAL_SECRETS_PASSPHRASE_LENGTH * 2,
            "passphrase too short: {}",
            pass.len()
        );
        assert!(pass.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn ephemeral_passphrase_is_random() {
        // Sanity-check that two consecutive draws differ.
        assert_ne!(ephemeral_passphrase(), ephemeral_passphrase());
    }

    /// Returns a `Config` configured for clone tests: a secrets encryption key is required
    /// so the ephemeral-passphrase round-trip in `clone_user_data` can encrypt/decrypt, and
    /// `retrack.host` is pointed at the supplied mock server so the export side's
    /// unconditional `GET /api/trackers` doesn't try to reach a real Retrack.
    ///
    /// The returned `MockServer` must be kept alive for the duration of the test; dropping
    /// it tears the listener down. We also pre-register a catch-all `GET /api/trackers`
    /// handler that returns an empty page, which is what every clone test in this file
    /// expects (none of them seed page/api trackers).
    fn clone_test_config() -> anyhow::Result<(crate::config::Config, MockServer)> {
        let mut config = mock_config()?;
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());

        let retrack_server = MockServer::start();
        retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Page::new(Vec::<Tracker>::new(), 0));
        });
        config.retrack.host = url::Url::parse(&retrack_server.base_url())?;

        Ok((config, retrack_server))
    }

    fn source_id() -> uuid::Uuid {
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
    }

    fn destination_id() -> uuid::Uuid {
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
    }

    /// Cloning an empty source user returns a summary with zero counts everywhere and
    /// touches no destination tables (the destination user is brand-new).
    #[sqlx::test]
    async fn clones_empty_user_with_zero_counts(pool: PgPool) -> anyhow::Result<()> {
        let (config, _retrack) = clone_test_config()?;
        let api = mock_api_with_config(pool, config).await?;
        let source = mock_user_with_id(source_id())?;
        let destination = mock_user_with_id(destination_id())?;
        api.db.insert_user(&source).await?;
        api.db.insert_user(&destination).await?;

        let summary = clone_user_data(&api, &source, &destination, false).await?;
        assert_eq!(summary.results.tags.imported, 0);
        assert_eq!(summary.results.scripts.imported, 0);
        assert_eq!(summary.results.secrets.imported, 0);
        assert_eq!(summary.results.responders.imported, 0);
        assert_eq!(summary.results.certificate_templates.imported, 0);
        assert_eq!(summary.results.private_keys.imported, 0);
        assert_eq!(summary.results.content_security_policies.imported, 0);
        assert_eq!(summary.results.page_trackers.imported, 0);
        assert_eq!(summary.results.api_trackers.imported, 0);

        Ok(())
    }

    /// Cloning a user with scripts and CSPs: counts match, IDs are regenerated, and the
    /// content of each entity is preserved verbatim on the destination side.
    #[sqlx::test]
    async fn clones_scripts_and_csps_under_new_ids(pool: PgPool) -> anyhow::Result<()> {
        let (config, _retrack) = clone_test_config()?;
        let api = mock_api_with_config(pool, config).await?;
        let source = mock_user_with_id(source_id())?;
        let destination = mock_user_with_id(destination_id())?;
        api.db.insert_user(&source).await?;
        api.db.insert_user(&destination).await?;

        let source_script = api
            .scripts(&source)
            .create_script(ScriptCreateParams {
                name: "responder_logic".into(),
                script_type: "responder".into(),
                content: "console.log('cloned')".into(),
                tag_ids: vec![],
            })
            .await?;

        let source_csp = api
            .web_security(&source)
            .create_content_security_policy(ContentSecurityPoliciesCreateParams {
                name: "strict".into(),
                content: ContentSecurityPolicyContent::Directives(vec![
                    ContentSecurityPolicyDirective::DefaultSrc(["'self'".into()].into()),
                ]),
                tag_ids: vec![],
            })
            .await?;

        let summary = clone_user_data(&api, &source, &destination, false).await?;
        assert_eq!(summary.results.scripts.imported, 1);
        assert_eq!(summary.results.content_security_policies.imported, 1);

        // The destination owns regenerated copies (new IDs, identical content).
        let dest_scripts = api.scripts(&destination).list_scripts(None).await?;
        assert_eq!(dest_scripts.len(), 1);
        assert_ne!(dest_scripts[0].id, source_script.id);
        assert_eq!(dest_scripts[0].name, "responder_logic");
        assert_eq!(dest_scripts[0].content, "console.log('cloned')");

        let dest_csps = api
            .web_security(&destination)
            .get_content_security_policies()
            .await?;
        assert_eq!(dest_csps.len(), 1);
        assert_ne!(dest_csps[0].id, source_csp.id);
        assert_eq!(dest_csps[0].name, "strict");

        // Source rows are untouched.
        assert_eq!(api.scripts(&source).list_scripts(None).await?.len(), 1);
        assert_eq!(
            api.web_security(&source)
                .get_content_security_policies()
                .await?
                .len(),
            1
        );

        Ok(())
    }

    /// Secrets are round-tripped through the export/import pipeline with an ephemeral
    /// passphrase so the destination row stores the plaintext re-encrypted under the
    /// destination's identity. Decrypting via the destination's secrets API must yield
    /// the original plaintext.
    #[sqlx::test]
    async fn clones_secrets_via_ephemeral_passphrase_round_trip(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let (config, _retrack) = clone_test_config()?;
        let api = mock_api_with_config(pool, config).await?;
        let source = mock_user_with_id(source_id())?;
        let destination = mock_user_with_id(destination_id())?;
        api.db.insert_user(&source).await?;
        api.db.insert_user(&destination).await?;

        api.secrets(&source)
            .create_secret(SecretCreateParams {
                name: "API_KEY".into(),
                value: "super-secret-value".into(),
                tag_ids: vec![],
            })
            .await?;

        let summary = clone_user_data(&api, &source, &destination, false).await?;
        assert_eq!(summary.results.secrets.imported, 1);
        assert_eq!(summary.results.secrets.failed, 0);

        // Source secret still works.
        let source_decrypted = api.secrets(&source).decrypt_all_secrets().await?;
        assert_eq!(
            source_decrypted.get("API_KEY").map(String::as_str),
            Some("super-secret-value")
        );

        // Destination row was re-encrypted under the global key and decrypts to the same
        // plaintext - this is the actual proof that the ephemeral passphrase round-trip
        // worked end-to-end.
        let dest_decrypted = api.secrets(&destination).decrypt_all_secrets().await?;
        assert_eq!(
            dest_decrypted.get("API_KEY").map(String::as_str),
            Some("super-secret-value")
        );

        Ok(())
    }

    /// `include_history=false` produces an export without history sub-payloads; the import
    /// side trivially honours that (responder requests / tracker revisions stay empty).
    /// We can't easily seed responder request history without a full webhooks setup, so
    /// this test just verifies the toggle deserialises cleanly into export selections and
    /// produces an empty-history summary on an empty source.
    #[sqlx::test]
    async fn include_history_toggle_compiles_into_export_selection(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let (config, _retrack) = clone_test_config()?;
        let api = mock_api_with_config(pool, config).await?;
        let source = mock_user_with_id(source_id())?;
        let destination = mock_user_with_id(destination_id())?;
        api.db.insert_user(&source).await?;
        api.db.insert_user(&destination).await?;

        // Both toggles must succeed on an empty source; the toggle changes only the
        // ExportTrackableSelection branch passed to generate_export.
        let with_history = clone_user_data(&api, &source, &destination, true).await?;
        assert_eq!(with_history.results.responders.imported, 0);

        // Clone again to the (now non-empty) destination; the destination still has no
        // responders, so a second clone is also a no-op on that axis.
        let again = clone_user_data(&api, &source, &destination, false).await?;
        assert_eq!(again.results.responders.imported, 0);

        Ok(())
    }

    /// `build_all_selections` should produce a selection per entity in the import file,
    /// regardless of how the source serialises them, and always marks them as `Import`.
    /// Verified end-to-end via a real export round-trip (which guarantees the same
    /// `UserDataImportFileData` shape the clone path actually sees in production).
    #[sqlx::test]
    async fn build_all_selections_includes_every_entity(pool: PgPool) -> anyhow::Result<()> {
        use crate::users::{
            UserDataExportParams,
            user_data::{
                export::{
                    ExportSelection, ExportTrackableSelection, UserDataExportInclude,
                    generate_export,
                },
                import::UserDataImportFile,
            },
        };

        let (config, _retrack) = clone_test_config()?;
        let api = mock_api_with_config(pool, config).await?;
        let source = mock_user_with_id(source_id())?;
        api.db.insert_user(&source).await?;

        // Seed exactly one entity so we can assert tags.len() == 1 below; the rest stay empty.
        api.scripts(&source)
            .create_script(ScriptCreateParams {
                name: "s1".into(),
                script_type: "responder".into(),
                content: "/* */".into(),
                tag_ids: vec![],
            })
            .await?;

        let export = generate_export(
            &api,
            &source,
            &UserDataExportParams {
                secrets_passphrase: None,
                include: UserDataExportInclude {
                    tags: Some(ExportSelection::All),
                    scripts: Some(ExportSelection::All),
                    secrets: Some(ExportSelection::All),
                    responders: Some(ExportTrackableSelection::All {
                        include_history: false,
                    }),
                    certificate_templates: Some(ExportSelection::All),
                    private_keys: Some(ExportSelection::All),
                    content_security_policies: Some(ExportSelection::All),
                    page_trackers: Some(ExportTrackableSelection::All {
                        include_history: false,
                    }),
                    api_trackers: Some(ExportTrackableSelection::All {
                        include_history: false,
                    }),
                    settings: true,
                },
            },
        )
        .await?;
        let import_file: UserDataImportFile =
            serde_json::from_value(serde_json::to_value(&export)?)?;

        let selections = build_all_selections(&import_file.data);
        assert_eq!(selections.scripts.len(), 1);
        assert!(matches!(selections.scripts[0].action, ImportAction::Import));
        assert!(selections.scripts[0].conflict_resolution.is_none());
        // import_settings should default to true so the source's user settings carry over.
        assert!(selections.import_settings);

        Ok(())
    }
}
