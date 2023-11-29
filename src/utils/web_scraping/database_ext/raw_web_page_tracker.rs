use crate::utils::{WebPageTracker, WebPageTrackerSettings, WebPageTrackerTag};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawWebPageTracker {
    pub id: Vec<u8>,
    pub name: String,
    pub url: String,
    pub kind: Vec<u8>,
    pub schedule: Option<String>,
    pub user_id: i64,
    pub job_id: Option<Vec<u8>>,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub(super) struct RawWebPageTrackerData<Tag: WebPageTrackerTag> {
    pub revisions: usize,
    pub delay: u64,
    pub scripts: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub enable_notifications: bool,
    pub meta: Option<Tag::TrackerMeta>,
}

impl<Tag: WebPageTrackerTag> TryFrom<RawWebPageTracker> for WebPageTracker<Tag> {
    type Error = anyhow::Error;

    fn try_from(raw: RawWebPageTracker) -> Result<Self, Self::Error> {
        let raw_data = postcard::from_bytes::<RawWebPageTrackerData<Tag>>(&raw.data)?;
        Ok(WebPageTracker {
            id: Uuid::from_slice(raw.id.as_slice())?,
            name: raw.name,
            url: raw.url.parse()?,
            user_id: raw.user_id.try_into()?,
            job_id: raw
                .job_id
                .map(|job_id| Uuid::from_slice(job_id.as_slice()))
                .transpose()?,
            settings: WebPageTrackerSettings {
                revisions: raw_data.revisions,
                delay: Duration::from_millis(raw_data.delay),
                schedule: raw.schedule,
                scripts: raw_data.scripts,
                headers: raw_data.headers,
                enable_notifications: raw_data.enable_notifications,
            },
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
            meta: raw_data.meta,
        })
    }
}

impl<Tag: WebPageTrackerTag> TryFrom<&WebPageTracker<Tag>> for RawWebPageTracker {
    type Error = anyhow::Error;

    fn try_from(item: &WebPageTracker<Tag>) -> Result<Self, Self::Error> {
        let raw_data = RawWebPageTrackerData::<Tag> {
            revisions: item.settings.revisions,
            delay: item.settings.delay.as_millis() as u64,
            scripts: item.settings.scripts.clone(),
            headers: item.settings.headers.clone(),
            enable_notifications: item.settings.enable_notifications,
            meta: item.meta.clone(),
        };

        Ok(RawWebPageTracker {
            id: item.id.into(),
            name: item.name.clone(),
            url: item.url.to_string(),
            kind: Tag::KIND.try_into()?,
            // Move schedule to a dedicated database table field to allow searching.
            schedule: item.settings.schedule.clone(),
            user_id: *item.user_id,
            job_id: item.job_id.as_ref().map(|job_id| (*job_id).into()),
            data: postcard::to_stdvec(&raw_data)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawWebPageTracker;
    use crate::{
        tests::mock_user,
        utils::{
            WebPageResourcesTrackerTag, WebPageTracker, WebPageTrackerSettings,
            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
        },
    };
    use std::time::Duration;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[test]
    fn can_convert_into_web_page_tracker() -> anyhow::Result<()> {
        assert_eq!(
            WebPageTracker::<WebPageResourcesTrackerTag>::try_from(RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                kind: vec![0],
                schedule: None,
                user_id: *mock_user()?.id,
                job_id: None,
                data: vec![1, 0, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: None,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    schedule: None,
                    delay: Default::default(),
                    scripts: Default::default(),
                    headers: Default::default(),
                    enable_notifications: false,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            }
        );

        assert_eq!(
            WebPageTracker::<WebPageResourcesTrackerTag>::try_from(RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                kind: vec![0],
                schedule: Some("0 0 * * *".to_string()),
                user_id: *mock_user()?.id,
                job_id: Some(
                    uuid!("00000000-0000-0000-0000-000000000002")
                        .as_bytes()
                        .to_vec()
                ),
                data: vec![
                    1, 208, 15, 1, 1, 17, 114, 101, 115, 111, 117, 114, 99, 101, 70, 105, 108, 116,
                    101, 114, 77, 97, 112, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115,
                    111, 117, 114, 99, 101, 59, 1, 1, 6, 99, 111, 111, 107, 105, 101, 9, 109, 121,
                    45, 99, 111, 111, 107, 105, 101, 1, 0
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string(),
                        )]
                        .into_iter()
                        .collect()
                    ),
                    headers: Some(
                        [("cookie".to_string(), "my-cookie".to_string())]
                            .into_iter()
                            .collect()
                    ),
                    enable_notifications: true,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_web_page_tracker() -> anyhow::Result<()> {
        assert_eq!(
            RawWebPageTracker::try_from(&WebPageTracker::<WebPageResourcesTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: None,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    schedule: None,
                    delay: Default::default(),
                    scripts: Default::default(),
                    headers: Default::default(),
                    enable_notifications: false,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            })?,
            RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                kind: vec![0],
                schedule: None,
                user_id: *mock_user()?.id,
                job_id: None,
                data: vec![1, 0, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        assert_eq!(
            RawWebPageTracker::try_from(&WebPageTracker::<WebPageResourcesTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string(),
                        )]
                        .into_iter()
                        .collect()
                    ),
                    headers: Some(
                        [("cookie".to_string(), "my-cookie".to_string())]
                            .into_iter()
                            .collect()
                    ),
                    enable_notifications: true,
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            })?,
            RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                kind: vec![0],
                schedule: Some("0 0 * * *".to_string()),
                user_id: *mock_user()?.id,
                job_id: Some(
                    uuid!("00000000-0000-0000-0000-000000000002")
                        .as_bytes()
                        .to_vec()
                ),
                data: vec![
                    1, 208, 15, 1, 1, 17, 114, 101, 115, 111, 117, 114, 99, 101, 70, 105, 108, 116,
                    101, 114, 77, 97, 112, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115,
                    111, 117, 114, 99, 101, 59, 1, 1, 6, 99, 111, 111, 107, 105, 101, 9, 109, 121,
                    45, 99, 111, 111, 107, 105, 101, 1, 0
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
