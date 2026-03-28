use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        settings::{UserSettings, UserSettingsSetter},
    },
};
use anyhow::bail;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub struct SettingsApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> SettingsApiExt<'a, 'u, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Retrieves the current settings for the user.
    pub async fn get_settings(&self) -> anyhow::Result<Option<UserSettings>> {
        self.api.db.get_user_settings(self.user.id).await
    }

    /// Validates and merges the setter into the existing settings.
    /// Returns the updated settings, or None if all settings were removed.
    pub async fn set_settings(
        &self,
        setter: UserSettingsSetter,
    ) -> anyhow::Result<Option<UserSettings>> {
        if !setter.is_valid() {
            bail!("User settings are not valid: {:?}", setter);
        }

        let mut merged: BTreeMap<String, serde_json::Value> = self
            .api
            .db
            .get_user_settings(self.user.id)
            .await?
            .map(|s| s.0)
            .unwrap_or_default();

        for (name, entry) in setter.into_inner() {
            if let Some(entry) = entry {
                merged.insert(name, entry);
            } else {
                merged.remove(&name);
            }
        }

        let now = OffsetDateTime::now_utc();
        if merged.is_empty() {
            self.api.db.remove_user_settings(self.user.id).await?;
            Ok(None)
        } else {
            let settings = UserSettings(merged);
            self.api
                .db
                .upsert_user_settings(self.user.id, &settings, now)
                .await?;
            Ok(Some(settings))
        }
    }

    /// Replaces settings wholesale (used by import). Does not validate against known settings
    /// since the export file may come from a different version.
    pub async fn replace_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        if settings.0.is_empty() {
            self.api.db.remove_user_settings(self.user.id).await
        } else {
            self.api
                .db
                .upsert_user_settings(self.user.id, settings, now)
                .await
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with user settings.
    pub fn settings<'a, 'u>(&'a self, user: &'u User) -> SettingsApiExt<'a, 'u, DR, ET> {
        SettingsApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::{UserSettings, UserSettingsSetter},
    };
    use serde_json::json;
    use sqlx::PgPool;
    use std::collections::BTreeMap;

    #[sqlx::test]
    async fn get_settings_returns_none_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let settings = api.settings(&user).get_settings().await?;
        assert!(settings.is_none());
        Ok(())
    }

    #[sqlx::test]
    async fn set_settings_merges_with_existing(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let settings_api = api.settings(&user);

        // Set initial settings.
        let setter = UserSettingsSetter(
            [
                ("common.uiTheme".to_string(), Some(json!("dark"))),
                (
                    "common.globalScopeTagIds".to_string(),
                    Some(json!(["tag-1"])),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let result = settings_api.set_settings(setter).await?.unwrap();
        assert_eq!(result.0.get("common.uiTheme").unwrap(), &json!("dark"));
        assert_eq!(
            result.0.get("common.globalScopeTagIds").unwrap(),
            &json!(["tag-1"])
        );

        // Merge: update one, remove one, add one.
        let setter = UserSettingsSetter(
            [
                ("common.uiTheme".to_string(), Some(json!("light"))),
                ("common.globalScopeTagIds".to_string(), None),
                ("common.sidebarCollapsed".to_string(), Some(json!(true))),
            ]
            .into_iter()
            .collect(),
        );
        let result = settings_api.set_settings(setter).await?.unwrap();
        assert_eq!(result.0.get("common.uiTheme").unwrap(), &json!("light"));
        assert!(!result.0.contains_key("common.globalScopeTagIds"));
        assert_eq!(
            result.0.get("common.sidebarCollapsed").unwrap(),
            &json!(true)
        );

        Ok(())
    }

    #[sqlx::test]
    async fn set_settings_removes_all_returns_none(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let settings_api = api.settings(&user);

        // Set one setting.
        let setter = UserSettingsSetter(
            [("common.uiTheme".to_string(), Some(json!("dark")))]
                .into_iter()
                .collect(),
        );
        settings_api.set_settings(setter).await?;

        // Remove it.
        let setter =
            UserSettingsSetter([("common.uiTheme".to_string(), None)].into_iter().collect());
        let result = settings_api.set_settings(setter).await?;
        assert!(result.is_none());
        assert!(settings_api.get_settings().await?.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn set_settings_rejects_invalid(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let setter = UserSettingsSetter(
            [("unknownSetting".to_string(), Some(json!(true)))]
                .into_iter()
                .collect(),
        );
        let err = api.settings(&user).set_settings(setter).await.unwrap_err();
        assert!(err.to_string().contains("not valid"));

        Ok(())
    }

    #[sqlx::test]
    async fn replace_settings_overwrites(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let settings_api = api.settings(&user);

        // Set initial settings via set_settings.
        let setter = UserSettingsSetter(
            [
                ("common.uiTheme".to_string(), Some(json!("dark"))),
                (
                    "common.globalScopeTagIds".to_string(),
                    Some(json!(["tag-1"])),
                ),
            ]
            .into_iter()
            .collect(),
        );
        settings_api.set_settings(setter).await?;

        // Replace with completely different settings.
        let new_settings = UserSettings(
            [("common.sidebarCollapsed".to_string(), json!(true))]
                .into_iter()
                .collect(),
        );
        settings_api.replace_settings(&new_settings).await?;

        let result = settings_api.get_settings().await?.unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(
            result.0.get("common.sidebarCollapsed").unwrap(),
            &json!(true)
        );
        assert!(!result.0.contains_key("common.uiTheme"));

        Ok(())
    }

    #[sqlx::test]
    async fn replace_settings_with_empty_removes(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let settings_api = api.settings(&user);

        let setter = UserSettingsSetter(
            [("common.uiTheme".to_string(), Some(json!("dark")))]
                .into_iter()
                .collect(),
        );
        settings_api.set_settings(setter).await?;

        settings_api
            .replace_settings(&UserSettings(BTreeMap::new()))
            .await?;
        assert!(settings_api.get_settings().await?.is_none());

        Ok(())
    }
}
