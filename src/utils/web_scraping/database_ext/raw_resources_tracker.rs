use crate::utils::{
    WebPageResourcesTracker, WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResourcesTracker {
    pub id: Vec<u8>,
    pub name: String,
    pub url: String,
    pub schedule: Option<String>,
    pub user_id: i64,
    pub job_id: Option<Vec<u8>>,
    pub settings: Vec<u8>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResourcesTrackerSettings {
    pub revisions: usize,
    pub delay: u64,
    pub resource_filter_map_script: Option<String>,
    pub enable_notifications: bool,
}

impl TryFrom<RawResourcesTracker> for WebPageResourcesTracker {
    type Error = anyhow::Error;

    fn try_from(raw: RawResourcesTracker) -> Result<Self, Self::Error> {
        let raw_settings = postcard::from_bytes::<RawResourcesTrackerSettings>(&raw.settings)?;
        Ok(WebPageResourcesTracker {
            id: Uuid::from_slice(raw.id.as_slice())?,
            name: raw.name,
            url: raw.url.parse()?,
            user_id: raw.user_id.try_into()?,
            job_id: raw
                .job_id
                .map(|job_id| Uuid::from_slice(job_id.as_slice()))
                .transpose()?,
            settings: WebPageResourcesTrackerSettings {
                revisions: raw_settings.revisions,
                delay: Duration::from_millis(raw_settings.delay),
                schedule: raw.schedule,
                scripts: WebPageResourcesTrackerScripts {
                    resource_filter_map: raw_settings.resource_filter_map_script,
                },
                enable_notifications: raw_settings.enable_notifications,
            },
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl TryFrom<&WebPageResourcesTracker> for RawResourcesTracker {
    type Error = anyhow::Error;

    fn try_from(item: &WebPageResourcesTracker) -> Result<Self, Self::Error> {
        let raw_settings = RawResourcesTrackerSettings {
            revisions: item.settings.revisions,
            delay: item.settings.delay.as_millis() as u64,
            resource_filter_map_script: item.settings.scripts.resource_filter_map.clone(),
            enable_notifications: item.settings.enable_notifications,
        };

        Ok(RawResourcesTracker {
            id: item.id.as_ref().to_vec(),
            name: item.name.clone(),
            url: item.url.to_string(),
            /// Move schedule to a dedicated database table field to allow searching.
            schedule: item.settings.schedule.clone(),
            user_id: *item.user_id,
            job_id: item.job_id.as_ref().map(|job_id| job_id.as_ref().to_vec()),
            settings: postcard::to_stdvec(&raw_settings)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawResourcesTracker;
    use crate::{
        tests::mock_user,
        utils::{
            WebPageResourcesTracker, WebPageResourcesTrackerScripts,
            WebPageResourcesTrackerSettings,
        },
    };
    use std::time::Duration;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[test]
    fn can_convert_into_resources_tracker() -> anyhow::Result<()> {
        assert_eq!(
            WebPageResourcesTracker::try_from(RawResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                schedule: None,
                user_id: *mock_user()?.id,
                job_id: None,
                settings: vec![1, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: None,
                settings: WebPageResourcesTrackerSettings {
                    revisions: 1,
                    schedule: None,
                    delay: Default::default(),
                    scripts: Default::default(),
                    enable_notifications: false,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        assert_eq!(
            WebPageResourcesTracker::try_from(RawResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                schedule: Some("0 0 * * *".to_string()),
                user_id: *mock_user()?.id,
                job_id: Some(
                    uuid!("00000000-0000-0000-0000-000000000002")
                        .as_bytes()
                        .to_vec()
                ),
                settings: vec![
                    1, 208, 15, 1, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115, 111, 117,
                    114, 99, 101, 59, 1
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                settings: WebPageResourcesTrackerSettings {
                    revisions: 1,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("return resource;".to_string()),
                    },
                    enable_notifications: true,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_resources_tracker() -> anyhow::Result<()> {
        assert_eq!(
            RawResourcesTracker::try_from(&WebPageResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: None,
                settings: WebPageResourcesTrackerSettings {
                    revisions: 1,
                    schedule: None,
                    delay: Default::default(),
                    scripts: Default::default(),
                    enable_notifications: false,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                schedule: None,
                user_id: *mock_user()?.id,
                job_id: None,
                settings: vec![1, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        assert_eq!(
            RawResourcesTracker::try_from(&WebPageResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                settings: WebPageResourcesTrackerSettings {
                    revisions: 1,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("return resource;".to_string()),
                    },
                    enable_notifications: true,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawResourcesTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                schedule: Some("0 0 * * *".to_string()),
                user_id: *mock_user()?.id,
                job_id: Some(
                    uuid!("00000000-0000-0000-0000-000000000002")
                        .as_bytes()
                        .to_vec()
                ),
                settings: vec![
                    1, 208, 15, 1, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115, 111, 117,
                    114, 99, 101, 59, 1
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
