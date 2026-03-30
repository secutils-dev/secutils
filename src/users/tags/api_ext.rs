use crate::{
    api::Api,
    error::Error,
    network::{DnsResolver, EmailTransport},
    users::{
        User,
        tags::{
            UserTag,
            user_tag::{
                MAX_TAG_NAME_LENGTH, MAX_TAGS_PER_USER, is_valid_tag_color, is_valid_tag_name,
                normalize_tag_name,
            },
        },
    },
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "production", "color": "primary"}))]
pub struct TagCreateParams {
    pub name: String,
    #[serde(default = "default_tag_color")]
    pub color: String,
}

fn default_tag_color() -> String {
    "default".to_string()
}

#[derive(Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"name": "staging", "color": "#54B399"}))]
pub struct TagUpdateParams {
    pub name: Option<String>,
    pub color: Option<String>,
}

pub struct TagsApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> TagsApiExt<'a, 'u, DR, ET> {
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Lists all tags for the user.
    pub async fn list_tags(&self) -> anyhow::Result<Vec<UserTag>> {
        self.api.db.get_user_tags(self.user.id).await
    }

    /// Fetches tags by a list of IDs.
    pub async fn bulk_get_tags(&self, ids: &[Uuid]) -> anyhow::Result<Vec<UserTag>> {
        self.api.db.bulk_get_user_tags(self.user.id, ids).await
    }

    /// Creates a new tag after validating name, color, and count limits.
    pub async fn create_tag(&self, params: TagCreateParams) -> anyhow::Result<UserTag> {
        let normalized_name = normalize_tag_name(&params.name);
        Self::validate_name(&normalized_name)?;
        Self::validate_color(&params.color)?;

        let count = self.api.db.count_user_tags(self.user.id).await?;
        if count as usize >= MAX_TAGS_PER_USER {
            return Err(anyhow::Error::from(Error::client(format!(
                "Maximum number of tags ({MAX_TAGS_PER_USER}) reached."
            ))));
        }

        self.api
            .db
            .insert_user_tag(self.user.id, &normalized_name, &params.color)
            .await
    }

    /// Updates an existing tag's name and/or color.
    pub async fn update_tag(&self, id: Uuid, params: TagUpdateParams) -> anyhow::Result<UserTag> {
        let normalized_name = params.name.as_deref().map(normalize_tag_name);
        if let Some(ref n) = normalized_name {
            Self::validate_name(n)?;
        }
        if let Some(ref c) = params.color {
            Self::validate_color(c)?;
        }

        self.api
            .db
            .update_user_tag(
                self.user.id,
                id,
                normalized_name.as_deref(),
                params.color.as_deref(),
            )
            .await?
            .ok_or_else(|| anyhow::Error::from(Error::not_found(format!("Tag '{id}' not found."))))
    }

    /// Deletes a tag by id. Junction rows are cascade-deleted.
    pub async fn delete_tag(&self, id: Uuid) -> anyhow::Result<UserTag> {
        self.api
            .db
            .remove_user_tag(self.user.id, id)
            .await?
            .ok_or_else(|| anyhow::Error::from(Error::not_found(format!("Tag '{id}' not found."))))
    }

    fn validate_name(name: &str) -> anyhow::Result<()> {
        if !is_valid_tag_name(name) {
            return Err(anyhow::Error::from(Error::client(format!(
                "Tag name must be non-empty and at most {MAX_TAG_NAME_LENGTH} characters."
            ))));
        }
        Ok(())
    }

    fn validate_color(color: &str) -> anyhow::Result<()> {
        if !is_valid_tag_color(color) {
            return Err(anyhow::Error::from(Error::client(format!(
                "Invalid tag color '{color}'. Use a hex color (#RGB or #RRGGBB) or a named color (default, primary, success, accent, warning, danger)."
            ))));
        }
        Ok(())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with user tags.
    pub fn tags<'a, 'u>(&'a self, user: &'u User) -> TagsApiExt<'a, 'u, DR, ET> {
        TagsApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use super::{TagCreateParams, TagUpdateParams};
    use crate::{
        tests::{mock_api, mock_user, schema_example},
        users::tags::user_tag::{is_valid_tag_color, is_valid_tag_name},
    };
    use sqlx::PgPool;

    #[test]
    fn tag_create_params_example_is_valid() {
        let example: TagCreateParams =
            serde_json::from_value(schema_example::<TagCreateParams>()).unwrap();
        assert!(is_valid_tag_name(&example.name));
        assert!(is_valid_tag_color(&example.color));
    }

    #[test]
    fn tag_update_params_example_is_valid() {
        let example: TagUpdateParams =
            serde_json::from_value(schema_example::<TagUpdateParams>()).unwrap();
        if let Some(ref name) = example.name {
            assert!(is_valid_tag_name(name));
        }
        if let Some(ref color) = example.color {
            assert!(is_valid_tag_color(color));
        }
    }

    #[sqlx::test]
    async fn list_tags_returns_empty_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags = api.tags(&mock_user).list_tags().await?;
        assert!(tags.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn create_tag_validates_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags_api = api.tags(&mock_user);

        let err = tags_api
            .create_tag(TagCreateParams {
                name: "".into(),
                color: "default".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Tag name must"));

        let err = tags_api
            .create_tag(TagCreateParams {
                name: "   ".into(),
                color: "default".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Tag name must"));

        let err = tags_api
            .create_tag(TagCreateParams {
                name: "a".repeat(51),
                color: "default".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Tag name must"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_tag_validates_color(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .tags(&mock_user)
            .create_tag(TagCreateParams {
                name: "test".into(),
                color: "purple".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Invalid tag color"));

        Ok(())
    }

    #[sqlx::test]
    async fn create_tag_normalizes_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tag = api
            .tags(&mock_user)
            .create_tag(TagCreateParams {
                name: "  Production  ".into(),
                color: "primary".into(),
            })
            .await?;
        assert_eq!(tag.name, "production");

        Ok(())
    }

    #[sqlx::test]
    async fn create_and_list_tags(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags_api = api.tags(&mock_user);
        let created = tags_api
            .create_tag(TagCreateParams {
                name: "production".into(),
                color: "primary".into(),
            })
            .await?;
        assert_eq!(created.name, "production");
        assert_eq!(created.color, "primary");

        let list = tags_api.list_tags().await?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "production");

        Ok(())
    }

    #[sqlx::test]
    async fn update_tag_changes_fields(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags_api = api.tags(&mock_user);
        let created = tags_api
            .create_tag(TagCreateParams {
                name: "old".into(),
                color: "default".into(),
            })
            .await?;

        let updated = tags_api
            .update_tag(
                created.id,
                TagUpdateParams {
                    name: Some("new".into()),
                    color: Some("danger".into()),
                },
            )
            .await?;
        assert_eq!(updated.name, "new");
        assert_eq!(updated.color, "danger");

        Ok(())
    }

    #[sqlx::test]
    async fn update_tag_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .tags(&mock_user)
            .update_tag(
                uuid::Uuid::now_v7(),
                TagUpdateParams {
                    name: Some("x".into()),
                    color: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn delete_tag_removes_it(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags_api = api.tags(&mock_user);
        let created = tags_api
            .create_tag(TagCreateParams {
                name: "to-delete".into(),
                color: "default".into(),
            })
            .await?;
        assert_eq!(tags_api.list_tags().await?.len(), 1);

        let deleted = tags_api.delete_tag(created.id).await?;
        assert_eq!(deleted.name, "to-delete");
        assert!(tags_api.list_tags().await?.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn delete_tag_not_found(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let err = api
            .tags(&mock_user)
            .delete_tag(uuid::Uuid::now_v7())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));

        Ok(())
    }

    #[sqlx::test]
    async fn bulk_get_tags_returns_selected(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.upsert_user(&mock_user).await?;

        let tags_api = api.tags(&mock_user);
        let tag_a = tags_api
            .create_tag(TagCreateParams {
                name: "alpha".into(),
                color: "primary".into(),
            })
            .await?;
        let tag_b = tags_api
            .create_tag(TagCreateParams {
                name: "beta".into(),
                color: "danger".into(),
            })
            .await?;
        tags_api
            .create_tag(TagCreateParams {
                name: "gamma".into(),
                color: "default".into(),
            })
            .await?;

        let result = tags_api.bulk_get_tags(&[tag_a.id, tag_b.id]).await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "alpha");
        assert_eq!(result[1].name, "beta");

        Ok(())
    }
}
