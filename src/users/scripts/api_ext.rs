use crate::{
    api::Api,
    error::Error,
    network::{DnsResolver, EmailTransport},
    users::{
        EntityTag, User,
        scripts::{ScriptContext, UserScript, UserScriptType},
    },
};
use serde::Deserialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "my-extractor", "scriptType": "api_extractor", "content": "export default async function() { return document.title; }", "tagIds": []}))]
pub struct ScriptCreateParams {
    pub name: String,
    pub script_type: String,
    pub content: String,
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
}

#[derive(Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"content": "export default async function() { return document.body.innerText; }"}))]
pub struct ScriptUpdateParams {
    pub content: String,
    pub tag_ids: Option<Vec<Uuid>>,
}

/// Maximum length for a script name.
const MAX_SCRIPT_NAME_LENGTH: usize = 128;
/// Maximum length for script content (50 KB).
const MAX_SCRIPT_CONTENT_LENGTH: usize = 50 * 1024;

pub struct ScriptsApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> ScriptsApiExt<'a, 'u, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Lists all scripts for the user, optionally filtered to those compatible with the given context.
    pub async fn list_scripts(
        &self,
        context: Option<ScriptContext>,
    ) -> anyhow::Result<Vec<UserScript>> {
        let scripts_db = self.api.db.scripts();
        let mut scripts = scripts_db.get_user_scripts(self.user.id).await?;
        let mut tags_map = scripts_db
            .get_script_tags(&scripts.iter().map(|s| s.id).collect::<Vec<_>>())
            .await?;
        for script in &mut scripts {
            script.tags = tags_map.remove(&script.id).unwrap_or_default();
        }
        Ok(match context {
            Some(ctx) => scripts
                .into_iter()
                .filter(|s| s.script_type.is_compatible_with(ctx))
                .collect(),
            None => scripts,
        })
    }

    /// Returns scripts with the specified IDs.
    pub async fn bulk_get_scripts(&self, ids: &[Uuid]) -> anyhow::Result<Vec<UserScript>> {
        let scripts_db = self.api.db.scripts();
        let mut scripts = scripts_db.bulk_get_user_scripts(self.user.id, ids).await?;
        let mut tags_map = scripts_db
            .get_script_tags(&scripts.iter().map(|s| s.id).collect::<Vec<_>>())
            .await?;
        for script in &mut scripts {
            script.tags = tags_map.remove(&script.id).unwrap_or_default();
        }
        Ok(scripts)
    }

    /// Gets a single script by id including its content.
    pub async fn get_script(&self, id: Uuid) -> anyhow::Result<Option<UserScript>> {
        let scripts_db = self.api.db.scripts();
        match scripts_db.get_user_script_by_id(self.user.id, id).await? {
            Some(mut script) => {
                script.tags = (scripts_db.get_script_tags(&[script.id]).await?)
                    .remove(&script.id)
                    .unwrap_or_default();
                Ok(Some(script))
            }
            None => Ok(None),
        }
    }

    /// Creates a new script after validating name, content, and subscription limits.
    pub async fn create_script(&self, params: ScriptCreateParams) -> anyhow::Result<UserScript> {
        Self::validate_name(&params.name)?;
        Self::validate_content(&params.content)?;
        Self::validate_script_type(&params.script_type)?;

        let scripts_db = self.api.db.scripts();
        let max_scripts = self
            .user
            .subscription
            .get_features(&self.api.config)
            .config
            .scripts
            .max_scripts;
        let count = scripts_db.count_user_scripts(self.user.id).await?;
        if count as usize >= max_scripts {
            return Err(anyhow::Error::from(Error::client(format!(
                "Maximum number of scripts ({max_scripts}) reached."
            ))));
        }

        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;
        let script = UserScript {
            id: Uuid::now_v7(),
            user_id: self.user.id,
            name: params.name,
            script_type: UserScriptType::from_str(&params.script_type)?,
            content: params.content,
            tags: params.tag_ids.into_iter().map(EntityTag::from).collect(),
            created_at,
            updated_at: created_at,
        };

        let tags = scripts_db.insert_user_script(self.user.id, &script).await?;

        Ok(UserScript { tags, ..script })
    }

    /// Updates an existing script's content.
    pub async fn update_script(
        &self,
        id: Uuid,
        params: ScriptUpdateParams,
    ) -> anyhow::Result<UserScript> {
        Self::validate_content(&params.content)?;

        let scripts_db = self.api.db.scripts();
        let existing_script = scripts_db
            .get_user_script_by_id(self.user.id, id)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("Script '{id}' not found.")))
            })?;

        let script = UserScript {
            content: params.content,
            // Preserve timestamp only up to seconds.
            updated_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            ..existing_script
        };

        let tags = scripts_db
            .update_user_script(self.user.id, &script, params.tag_ids)
            .await?;

        Ok(if let Some(updated_tags) = tags {
            UserScript {
                tags: updated_tags,
                ..script
            }
        } else {
            script
        })
    }

    /// Deletes a script by id.
    pub async fn delete_script(&self, id: Uuid) -> anyhow::Result<UserScript> {
        let script = self
            .api
            .db
            .scripts()
            .remove_user_script(self.user.id, id)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("Script '{id}' not found.")))
            })?;

        Ok(script)
    }

    fn validate_name(name: &str) -> anyhow::Result<()> {
        if name.trim().is_empty() {
            return Err(anyhow::Error::from(Error::client(
                "Script name cannot be empty.",
            )));
        }
        if name.len() > MAX_SCRIPT_NAME_LENGTH {
            return Err(anyhow::Error::from(Error::client(format!(
                "Script name cannot be longer than {MAX_SCRIPT_NAME_LENGTH} characters."
            ))));
        }
        Ok(())
    }

    fn validate_content(content: &str) -> anyhow::Result<()> {
        if content.is_empty() {
            return Err(anyhow::Error::from(Error::client(
                "Script content cannot be empty.",
            )));
        }
        if content.len() > MAX_SCRIPT_CONTENT_LENGTH {
            return Err(anyhow::Error::from(Error::client(format!(
                "Script content must be at most {} bytes.",
                MAX_SCRIPT_CONTENT_LENGTH
            ))));
        }
        Ok(())
    }

    fn validate_script_type(script_type: &str) -> anyhow::Result<()> {
        match script_type {
            "responder" | "api_configurator" | "api_extractor" | "page_extractor" | "universal" => {
                Ok(())
            }
            _ => Err(anyhow::Error::from(Error::client(
                "Invalid script type. Must be one of: responder, api_configurator, api_extractor, page_extractor, universal",
            ))),
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with user scripts.
    pub fn scripts<'a, 'u>(&'a self, user: &'u User) -> ScriptsApiExt<'a, 'u, DR, ET> {
        ScriptsApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_SCRIPT_CONTENT_LENGTH, MAX_SCRIPT_NAME_LENGTH};
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::scripts::{ScriptCreateParams, ScriptUpdateParams, UserScriptType},
    };
    use sqlx::PgPool;
    use utoipa::PartialSchema;

    fn schema_example<T: PartialSchema>() -> serde_json::Value {
        match T::schema() {
            utoipa::openapi::RefOr::T(schema) => match schema {
                utoipa::openapi::Schema::Object(obj) => obj.example.expect("schema has no example"),
                _ => panic!("expected Object schema"),
            },
            utoipa::openapi::RefOr::Ref(_) => panic!("expected inline schema, got Ref"),
        }
    }

    #[test]
    fn script_create_params_example_is_valid() {
        let example: ScriptCreateParams =
            serde_json::from_value(schema_example::<ScriptCreateParams>()).unwrap();
        assert!(!example.name.trim().is_empty());
        assert!(example.name.len() <= MAX_SCRIPT_NAME_LENGTH);
        assert!(!example.content.is_empty());
        assert!(example.content.len() <= MAX_SCRIPT_CONTENT_LENGTH);
        // Validate script_type is one of the known types.
        assert!(
            [
                "responder",
                "api_configurator",
                "api_extractor",
                "page_extractor",
                "universal"
            ]
            .contains(&example.script_type.as_str())
        );
    }

    #[test]
    fn script_update_params_example_is_valid() {
        let example: ScriptUpdateParams =
            serde_json::from_value(schema_example::<ScriptUpdateParams>()).unwrap();
        assert!(!example.content.is_empty());
        assert!(example.content.len() <= MAX_SCRIPT_CONTENT_LENGTH);
    }

    #[sqlx::test]
    async fn list_scripts_filters_by_context(pool: PgPool) -> anyhow::Result<()> {
        use crate::users::scripts::ScriptContext;

        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        scripts_api
            .create_script(ScriptCreateParams {
                name: "responder_script".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;
        scripts_api
            .create_script(ScriptCreateParams {
                name: "api_configurator_script".into(),
                script_type: "api_configurator".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;
        scripts_api
            .create_script(ScriptCreateParams {
                name: "api_extractor_script".into(),
                script_type: "api_extractor".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;
        scripts_api
            .create_script(ScriptCreateParams {
                name: "page_extractor_script".into(),
                script_type: "page_extractor".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;
        scripts_api
            .create_script(ScriptCreateParams {
                name: "universal_script".into(),
                script_type: "universal".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;

        let responder_scripts = scripts_api
            .list_scripts(Some(ScriptContext::Responder))
            .await?;
        assert_eq!(responder_scripts.len(), 2);
        assert!(
            responder_scripts
                .iter()
                .any(|s| s.name == "responder_script")
        );
        assert!(
            responder_scripts
                .iter()
                .any(|s| s.name == "universal_script")
        );

        let api_tracker_scripts = scripts_api
            .list_scripts(Some(ScriptContext::ApiTracker))
            .await?;
        assert_eq!(api_tracker_scripts.len(), 3);
        assert!(
            api_tracker_scripts
                .iter()
                .any(|s| s.name == "api_configurator_script")
        );
        assert!(
            api_tracker_scripts
                .iter()
                .any(|s| s.name == "api_extractor_script")
        );
        assert!(
            api_tracker_scripts
                .iter()
                .any(|s| s.name == "universal_script")
        );

        let page_tracker_scripts = scripts_api
            .list_scripts(Some(ScriptContext::PageTracker))
            .await?;
        assert_eq!(page_tracker_scripts.len(), 2);
        assert!(
            page_tracker_scripts
                .iter()
                .any(|s| s.name == "page_extractor_script")
        );
        assert!(
            page_tracker_scripts
                .iter()
                .any(|s| s.name == "universal_script")
        );

        let all_scripts = scripts_api.list_scripts(None).await?;
        assert_eq!(all_scripts.len(), 5);

        Ok(())
    }

    #[sqlx::test]
    async fn list_scripts_returns_empty_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts = api.scripts(&mock_user).list_scripts(None).await?;
        assert!(scripts.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn create_script_validates_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script name cannot be empty"));

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "   ".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script name cannot be empty"));

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "a".repeat(129),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Script name cannot be longer than")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn create_script_validates_content(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "VALID_NAME".into(),
                script_type: "responder".into(),
                content: "".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script content cannot be empty"));

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "VALID_NAME".into(),
                script_type: "responder".into(),
                content: "x".repeat(50 * 1024 + 1),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script content must be at most"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_script_validates_script_type(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "VALID_NAME".into(),
                script_type: "invalid_type".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid script type"));

        // Valid script types should work
        for script_type in [
            "responder",
            "api_configurator",
            "api_extractor",
            "page_extractor",
            "universal",
        ] {
            let name = format!("script_{}", script_type);
            let script = scripts_api
                .create_script(ScriptCreateParams {
                    name: name.clone(),
                    script_type: script_type.into(),
                    content: "content".into(),
                    tag_ids: vec![],
                })
                .await?;
            assert_eq!(script.name, name);
        }

        Ok(())
    }

    #[sqlx::test]
    async fn create_script_enforces_limit(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        config.subscriptions.ultimate.scripts.max_scripts = 2;
        let api = mock_api_with_config(pool, config).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        scripts_api
            .create_script(ScriptCreateParams {
                name: "SCRIPT_A".into(),
                script_type: "responder".into(),
                content: "content-a".into(),
                tag_ids: vec![],
            })
            .await?;
        scripts_api
            .create_script(ScriptCreateParams {
                name: "SCRIPT_B".into(),
                script_type: "responder".into(),
                content: "content-b".into(),
                tag_ids: vec![],
            })
            .await?;

        let err = scripts_api
            .create_script(ScriptCreateParams {
                name: "SCRIPT_C".into(),
                script_type: "responder".into(),
                content: "content-c".into(),
                tag_ids: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Maximum number of scripts (2)"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_and_list_scripts(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        let created = scripts_api
            .create_script(ScriptCreateParams {
                name: "MY_SCRIPT".into(),
                script_type: "responder".into(),
                content: "console.log('hello');".into(),
                tag_ids: vec![],
            })
            .await?;
        assert_eq!(created.name, "MY_SCRIPT");
        assert_eq!(created.script_type, UserScriptType::Responder);
        assert_eq!(created.content, "console.log('hello');");

        let list = scripts_api.list_scripts(None).await?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "MY_SCRIPT");
        assert_eq!(list[0].script_type, UserScriptType::Responder);

        Ok(())
    }

    #[sqlx::test]
    async fn get_script_returns_content(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);

        // Non-existent script
        let script = scripts_api.get_script(uuid::Uuid::now_v7()).await?;
        assert!(script.is_none());

        // Create and retrieve
        let created = scripts_api
            .create_script(ScriptCreateParams {
                name: "TEST_SCRIPT".into(),
                script_type: "api_extractor".into(),
                content: "return data;".into(),
                tag_ids: vec![],
            })
            .await?;

        let script = scripts_api.get_script(created.id).await?.unwrap();
        assert_eq!(script.name, "TEST_SCRIPT");
        assert_eq!(script.content, "return data;");
        assert_eq!(script.script_type, UserScriptType::ApiExtractor);

        Ok(())
    }

    #[sqlx::test]
    async fn update_script_changes_content(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        let created = scripts_api
            .create_script(ScriptCreateParams {
                name: "MY_SCRIPT".into(),
                script_type: "responder".into(),
                content: "old-content".into(),
                tag_ids: vec![],
            })
            .await?;

        let updated = scripts_api
            .update_script(
                created.id,
                ScriptUpdateParams {
                    content: "new-content".into(),
                    tag_ids: None,
                },
            )
            .await?;
        assert_eq!(updated.name, "MY_SCRIPT");
        assert_eq!(updated.content, "new-content");

        // Verify the update persisted
        let script = scripts_api.get_script(created.id).await?.unwrap();
        assert_eq!(script.content, "new-content");

        Ok(())
    }

    #[sqlx::test]
    async fn update_script_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .scripts(&mock_user)
            .update_script(
                uuid::Uuid::now_v7(),
                ScriptUpdateParams {
                    content: "content".into(),
                    tag_ids: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn delete_script_removes_it(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        let created = scripts_api
            .create_script(ScriptCreateParams {
                name: "TO_DELETE".into(),
                script_type: "responder".into(),
                content: "content".into(),
                tag_ids: vec![],
            })
            .await?;
        assert_eq!(scripts_api.list_scripts(None).await?.len(), 1);

        let deleted = scripts_api.delete_script(created.id).await?;
        assert_eq!(deleted.name, "TO_DELETE");
        assert!(scripts_api.list_scripts(None).await?.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn delete_script_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .scripts(&mock_user)
            .delete_script(uuid::Uuid::now_v7())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }
}
