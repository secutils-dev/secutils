use crate::{retrack::RetrackTracker, utils::web_scraping::PageTracker};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawPageTracker {
    pub id: Uuid,
    pub name: String,
    pub user_id: Uuid,
    pub retrack_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TryFrom<RawPageTracker> for PageTracker {
    type Error = anyhow::Error;

    fn try_from(raw: RawPageTracker) -> Result<Self, Self::Error> {
        Ok(PageTracker {
            id: raw.id,
            name: raw.name,
            user_id: raw.user_id.into(),
            retrack: RetrackTracker::from_reference(raw.retrack_id),
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

impl TryFrom<&PageTracker> for RawPageTracker {
    type Error = anyhow::Error;

    fn try_from(item: &PageTracker) -> Result<Self, Self::Error> {
        Ok(RawPageTracker {
            id: item.id,
            name: item.name.clone(),
            user_id: *item.user_id,
            retrack_id: item.retrack.id(),
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawPageTracker;
    use crate::{retrack::RetrackTracker, tests::mock_user, utils::web_scraping::PageTracker};
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_page_tracker() -> anyhow::Result<()> {
        assert_eq!(
            PageTracker::try_from(RawPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: *mock_user()?.id,
                retrack_id: uuid!("00000000-0000-0000-0000-000000000002"),
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            PageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: mock_user()?.id,
                retrack: RetrackTracker::Reference {
                    id: uuid!("00000000-0000-0000-0000-000000000002")
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_page_tracker() -> anyhow::Result<()> {
        assert_eq!(
            RawPageTracker::try_from(&PageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: mock_user()?.id,
                retrack: RetrackTracker::Reference {
                    id: uuid!("00000000-0000-0000-0000-000000000002")
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?
            })?,
            RawPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                user_id: *mock_user()?.id,
                retrack_id: uuid!("00000000-0000-0000-0000-000000000002"),
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }
}
