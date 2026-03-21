use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        user_data::import::{
            ConflictResolution, ImportEntityResult, ImportEntitySelection, ImportedScript,
            resolve_name, should_skip,
        },
    },
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub async fn import_scripts<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    scripts: &[ImportedScript],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
) -> ImportEntityResult {
    let mut result = ImportEntityResult::default();

    // Pre-fetch existing scripts once for overwritten resolution.
    let existing_scripts = api
        .scripts(user)
        .list_scripts(None)
        .await
        .unwrap_or_default();
    let mut used_names: HashSet<String> = existing_scripts.iter().map(|s| s.name.clone()).collect();

    for script in scripts {
        let selection = selections.get(&script.id);
        if should_skip(selection) {
            result.skipped += 1;
            continue;
        }

        let name = resolve_name(&script.name, selection, &used_names);

        // Handle overwrite: delete existing with the same name.
        if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Overwrite))
            && let Some(e) = existing_scripts.iter().find(|s| s.name == script.name)
        {
            let _ = api.scripts(user).delete_script(e.id).await;
            used_names.remove(&script.name);
        }

        match api
            .scripts(user)
            .create_script(&name, &script.script_type, &script.content)
            .await
        {
            Ok(_) => {
                used_names.insert(name);
                result.imported += 1;
            }
            Err(err) => {
                result.failed += 1;
                result
                    .errors
                    .push(format!("Script '{}': {}", script.name, err));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        file::{ImportedScript, UserDataImportFile, UserDataImportFileData},
        params::{
            ConflictResolution, ImportAction, ImportEntitySelection, ImportMode, ImportSelections,
            UserDataImportParams,
        },
    };
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::user_data::import::execute_import,
    };
    use sqlx::PgPool;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_scripts_file(scripts: Vec<ImportedScript>) -> UserDataImportFile {
        UserDataImportFile {
            version: 1,
            exported_at: datetime!(2020-01-01 12:00:00 UTC),
            secrets_encryption: None,
            data: UserDataImportFileData {
                scripts,
                secrets: vec![],
                responders: vec![],
                certificate_templates: vec![],
                private_keys: vec![],
                content_security_policies: vec![],
                page_trackers: vec![],
                api_trackers: vec![],
            },
        }
    }

    #[sqlx::test]
    async fn import_scripts_merge(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let script_id = Uuid::now_v7();
        let file = make_scripts_file(vec![ImportedScript {
            id: script_id,
            name: "imported_script".to_string(),
            script_type: "responder".to_string(),
            content: "console.log('imported')".to_string(),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                scripts: vec![ImportEntitySelection {
                    source_id: script_id,
                    action: ImportAction::Import,
                    conflict_resolution: None,
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.scripts.imported, 1);
        assert_eq!(result.results.scripts.failed, 0);

        let scripts = api.scripts(&user).list_scripts(None).await?;
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "imported_script");

        Ok(())
    }

    #[sqlx::test]
    async fn import_scripts_with_rename_conflict(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        api.scripts(&user)
            .create_script("my_script", "responder", "original")
            .await?;

        let script_id = Uuid::now_v7();
        let file = make_scripts_file(vec![ImportedScript {
            id: script_id,
            name: "my_script".to_string(),
            script_type: "responder".to_string(),
            content: "new content".to_string(),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }]);

        let params = UserDataImportParams {
            data: file,
            mode: ImportMode::Merge,
            secrets_passphrase: None,
            apply_deletions: None,
            selections: ImportSelections {
                scripts: vec![ImportEntitySelection {
                    source_id: script_id,
                    action: ImportAction::Import,
                    conflict_resolution: Some(ConflictResolution::Rename),
                }],
                ..Default::default()
            },
        };

        let result = execute_import(&api, &user, params).await?;
        assert_eq!(result.results.scripts.imported, 1);

        let scripts = api.scripts(&user).list_scripts(None).await?;
        assert_eq!(scripts.len(), 2);
        let names: Vec<&str> = scripts.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"my_script"));
        assert!(names.contains(&"my_script (Copy 1)"));

        Ok(())
    }
}
