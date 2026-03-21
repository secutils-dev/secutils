use crate::{
    api::Api,
    error::Error,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        scripts::{ScriptContext, UserScript},
    },
};
use uuid::Uuid;

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
        let scripts = self.api.db.get_user_scripts(self.user.id).await?;
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
        self.api.db.bulk_get_user_scripts(self.user.id, ids).await
    }

    /// Gets a single script by id including its content.
    pub async fn get_script(&self, id: Uuid) -> anyhow::Result<Option<UserScript>> {
        self.api.db.get_user_script_by_id(self.user.id, id).await
    }

    /// Creates a new script after validating name, content, and subscription limits.
    pub async fn create_script(
        &self,
        name: &str,
        script_type: &str,
        content: &str,
    ) -> anyhow::Result<UserScript> {
        Self::validate_name(name)?;
        Self::validate_content(content)?;
        Self::validate_script_type(script_type)?;

        let max_scripts = self
            .user
            .subscription
            .get_features(&self.api.config)
            .config
            .scripts
            .max_scripts;
        let count = self.api.db.count_user_scripts(self.user.id).await?;
        if count as usize >= max_scripts {
            return Err(anyhow::Error::from(Error::client(format!(
                "Maximum number of scripts ({max_scripts}) reached."
            ))));
        }

        let script = self
            .api
            .db
            .insert_user_script(self.user.id, name, script_type, content)
            .await?;

        Ok(script)
    }

    /// Updates an existing script's content.
    pub async fn update_script(&self, id: Uuid, content: &str) -> anyhow::Result<UserScript> {
        Self::validate_content(content)?;

        let script = self
            .api
            .db
            .update_user_script(self.user.id, id, content)
            .await?
            .ok_or_else(|| {
                anyhow::Error::from(Error::not_found(format!("Script '{id}' not found.")))
            })?;

        Ok(script)
    }

    /// Deletes a script by id.
    pub async fn delete_script(&self, id: Uuid) -> anyhow::Result<UserScript> {
        let script = self
            .api
            .db
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
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::scripts::UserScriptType,
    };
    use sqlx::PgPool;

    #[sqlx::test]
    async fn list_scripts_filters_by_context(pool: PgPool) -> anyhow::Result<()> {
        use crate::users::scripts::ScriptContext;

        let api = mock_api_with_config(pool, mock_config()?).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let scripts_api = api.scripts(&mock_user);
        scripts_api
            .create_script("responder_script", "responder", "content")
            .await?;
        scripts_api
            .create_script("api_configurator_script", "api_configurator", "content")
            .await?;
        scripts_api
            .create_script("api_extractor_script", "api_extractor", "content")
            .await?;
        scripts_api
            .create_script("page_extractor_script", "page_extractor", "content")
            .await?;
        scripts_api
            .create_script("universal_script", "universal", "content")
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
            .create_script("", "responder", "content")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script name cannot be empty"));

        let err = scripts_api
            .create_script("   ", "responder", "content")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script name cannot be empty"));

        let err = scripts_api
            .create_script(&"a".repeat(129), "responder", "content")
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
            .create_script("VALID_NAME", "responder", "")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Script content cannot be empty"));

        let err = scripts_api
            .create_script("VALID_NAME", "responder", &"x".repeat(50 * 1024 + 1))
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
            .create_script("VALID_NAME", "invalid_type", "content")
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
                .create_script(&name, script_type, "content")
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
            .create_script("SCRIPT_A", "responder", "content-a")
            .await?;
        scripts_api
            .create_script("SCRIPT_B", "responder", "content-b")
            .await?;

        let err = scripts_api
            .create_script("SCRIPT_C", "responder", "content-c")
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
            .create_script("MY_SCRIPT", "responder", "console.log('hello');")
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
            .create_script("TEST_SCRIPT", "api_extractor", "return data;")
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
            .create_script("MY_SCRIPT", "responder", "old-content")
            .await?;

        let updated = scripts_api.update_script(created.id, "new-content").await?;
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
            .update_script(uuid::Uuid::now_v7(), "content")
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
            .create_script("TO_DELETE", "responder", "content")
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
