use crate::{retrack::RetrackTracker, users::RawSecretsAccess, utils::web_scraping::ApiTracker};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone, sqlx::FromRow)]
pub(super) struct RawApiTracker {
    pub id: Uuid,
    pub name: String,
    pub user_id: Uuid,
    pub retrack_id: Uuid,
    pub secrets: Vec<u8>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TryFrom<RawApiTracker> for ApiTracker {
    type Error = anyhow::Error;

    fn try_from(raw: RawApiTracker) -> Result<Self, Self::Error> {
        Ok(ApiTracker {
            id: raw.id,
            name: raw.name,
            user_id: raw.user_id.into(),
            retrack: RetrackTracker::from_reference(raw.retrack_id),
            secrets: postcard::from_bytes::<RawSecretsAccess>(&raw.secrets)
                .map(Into::into)
                .unwrap_or_default(),
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

impl TryFrom<&ApiTracker> for RawApiTracker {
    type Error = anyhow::Error;

    fn try_from(item: &ApiTracker) -> Result<Self, Self::Error> {
        Ok(RawApiTracker {
            id: item.id,
            name: item.name.clone(),
            user_id: *item.user_id,
            retrack_id: item.retrack.id(),
            secrets: postcard::to_stdvec(&RawSecretsAccess::from(&item.secrets))?,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawApiTracker;
    use crate::{
        retrack::RetrackTracker, tests::mock_user, users::SecretsAccess,
        utils::web_scraping::ApiTracker,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_api_tracker() -> anyhow::Result<()> {
        assert_eq!(
            ApiTracker::try_from(RawApiTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: *mock_user()?.id,
                retrack_id: uuid!("00000000-0000-0000-0000-000000000002"),
                secrets: vec![0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            ApiTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: mock_user()?.id,
                retrack: RetrackTracker::Reference {
                    id: uuid!("00000000-0000-0000-0000-000000000002")
                },
                secrets: SecretsAccess::None,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_api_tracker() -> anyhow::Result<()> {
        assert_eq!(
            RawApiTracker::try_from(&ApiTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: mock_user()?.id,
                retrack: RetrackTracker::Reference {
                    id: uuid!("00000000-0000-0000-0000-000000000002")
                },
                secrets: SecretsAccess::None,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?
            })?,
            RawApiTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: *mock_user()?.id,
                retrack_id: uuid!("00000000-0000-0000-0000-000000000002"),
                secrets: vec![0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }
}
