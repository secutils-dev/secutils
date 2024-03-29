use crate::{
    scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
    utils::web_scraping::{WebPageTracker, WebPageTrackerSettings, WebPageTrackerTag},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawWebPageTracker {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub kind: Vec<u8>,
    pub user_id: i32,
    pub job_id: Option<Uuid>,
    pub job_config: Option<Vec<u8>>,
    pub data: Vec<u8>,
    pub created_at: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub(super) struct RawWebPageTrackerData<Tag: WebPageTrackerTag> {
    pub revisions: usize,
    pub delay: u64,
    pub scripts: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub meta: Option<Tag::TrackerMeta>,
}

#[derive(Serialize, Deserialize)]
struct RawSchedulerJobConfig(String, Option<RawSchedulerJobRetryStrategy>, bool);

#[derive(Serialize, Deserialize)]
enum RawSchedulerJobRetryStrategy {
    Constant(Duration, u32),
    Exponential(Duration, u32, Duration, u32),
    Linear(Duration, Duration, Duration, u32),
}

impl<Tag: WebPageTrackerTag> TryFrom<RawWebPageTracker> for WebPageTracker<Tag> {
    type Error = anyhow::Error;

    fn try_from(raw: RawWebPageTracker) -> Result<Self, Self::Error> {
        let raw_data = postcard::from_bytes::<RawWebPageTrackerData<Tag>>(&raw.data)?;

        let job_config = if let Some(job_config) = raw.job_config {
            let RawSchedulerJobConfig(schedule, retry_strategy, notifications) =
                postcard::from_bytes(&job_config)?;
            Some(SchedulerJobConfig {
                schedule,
                retry_strategy: retry_strategy.map(|retry_strategy| match retry_strategy {
                    RawSchedulerJobRetryStrategy::Constant(interval, max_attempts) => {
                        SchedulerJobRetryStrategy::Constant {
                            interval,
                            max_attempts,
                        }
                    }
                    RawSchedulerJobRetryStrategy::Exponential(
                        initial_interval,
                        multiplier,
                        max_interval,
                        max_attempts,
                    ) => SchedulerJobRetryStrategy::Exponential {
                        initial_interval,
                        multiplier,
                        max_interval,
                        max_attempts,
                    },
                    RawSchedulerJobRetryStrategy::Linear(
                        initial_interval,
                        increment,
                        max_interval,
                        max_attempts,
                    ) => SchedulerJobRetryStrategy::Linear {
                        initial_interval,
                        increment,
                        max_interval,
                        max_attempts,
                    },
                }),
                notifications,
            })
        } else {
            None
        };

        Ok(WebPageTracker {
            id: raw.id,
            name: raw.name,
            url: raw.url.parse()?,
            user_id: raw.user_id.try_into()?,
            job_id: raw.job_id,
            job_config,
            settings: WebPageTrackerSettings {
                revisions: raw_data.revisions,
                delay: Duration::from_millis(raw_data.delay),
                scripts: raw_data.scripts,
                headers: raw_data.headers,
            },
            created_at: raw.created_at,
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
            meta: item.meta.clone(),
        };

        let job_config = if let Some(SchedulerJobConfig {
            schedule,
            retry_strategy,
            notifications,
        }) = &item.job_config
        {
            Some(postcard::to_stdvec(&RawSchedulerJobConfig(
                schedule.to_string(),
                retry_strategy.map(|retry_strategy| match retry_strategy {
                    SchedulerJobRetryStrategy::Constant {
                        interval,
                        max_attempts,
                    } => RawSchedulerJobRetryStrategy::Constant(interval, max_attempts),
                    SchedulerJobRetryStrategy::Exponential {
                        initial_interval,
                        multiplier,
                        max_interval,
                        max_attempts,
                    } => RawSchedulerJobRetryStrategy::Exponential(
                        initial_interval,
                        multiplier,
                        max_interval,
                        max_attempts,
                    ),
                    SchedulerJobRetryStrategy::Linear {
                        initial_interval,
                        increment,
                        max_interval,
                        max_attempts,
                    } => RawSchedulerJobRetryStrategy::Linear(
                        initial_interval,
                        increment,
                        max_interval,
                        max_attempts,
                    ),
                }),
                *notifications,
            ))?)
        } else {
            None
        };

        Ok(RawWebPageTracker {
            id: item.id,
            name: item.name.clone(),
            url: item.url.to_string(),
            kind: Tag::KIND.try_into()?,
            user_id: *item.user_id,
            job_id: item.job_id,
            job_config,
            data: postcard::to_stdvec(&raw_data)?,
            created_at: item.created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawWebPageTracker;
    use crate::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        tests::mock_user,
        utils::web_scraping::{
            api_ext::WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME, WebPageResourcesTrackerTag,
            WebPageTracker, WebPageTrackerSettings,
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
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                kind: vec![0],
                user_id: *mock_user()?.id,
                job_id: None,
                job_config: None,
                data: vec![1, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            WebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: None,
                job_config: None,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Default::default(),
                    scripts: Default::default(),
                    headers: Default::default()
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            }
        );

        assert_eq!(
            WebPageTracker::<WebPageResourcesTrackerTag>::try_from(RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: "https://secutils.dev".to_string(),
                kind: vec![0],
                user_id: *mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                job_config: Some(vec![
                    9, 48, 32, 48, 32, 42, 32, 42, 32, 42, 1, 1, 1, 128, 157, 202, 111, 2, 120, 0,
                    5, 1
                ]),
                data: vec![
                    1, 208, 15, 1, 1, 17, 114, 101, 115, 111, 117, 114, 99, 101, 70, 105, 108, 116,
                    101, 114, 77, 97, 112, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115,
                    111, 117, 114, 99, 101, 59, 1, 1, 6, 99, 111, 111, 107, 105, 101, 9, 109, 121,
                    45, 99, 111, 111, 107, 105, 101, 0
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            WebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                        initial_interval: Duration::from_millis(1234),
                        multiplier: 2,
                        max_interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: true
                }),
                settings: WebPageTrackerSettings {
                    revisions: 1,
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
                    )
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
                job_config: None,
                settings: WebPageTrackerSettings {
                    revisions: 1,
                    delay: Default::default(),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            })?,
            RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                kind: vec![0],
                user_id: *mock_user()?.id,
                job_id: None,
                job_config: None,
                data: vec![1, 0, 0, 0, 0],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        assert_eq!(
            RawWebPageTracker::try_from(&WebPageTracker::<WebPageResourcesTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                user_id: mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                        initial_interval: Duration::from_millis(1234),
                        multiplier: 2,
                        max_interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: true
                }),
                settings: WebPageTrackerSettings {
                    revisions: 1,
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
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                meta: None
            })?,
            RawWebPageTracker {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "tk".to_string(),
                url: "https://secutils.dev/".to_string(),
                kind: vec![0],
                user_id: *mock_user()?.id,
                job_id: Some(uuid!("00000000-0000-0000-0000-000000000002")),
                job_config: Some(vec![
                    9, 48, 32, 48, 32, 42, 32, 42, 32, 42, 1, 1, 1, 128, 157, 202, 111, 2, 120, 0,
                    5, 1
                ]),
                data: vec![
                    1, 208, 15, 1, 1, 17, 114, 101, 115, 111, 117, 114, 99, 101, 70, 105, 108, 116,
                    101, 114, 77, 97, 112, 16, 114, 101, 116, 117, 114, 110, 32, 114, 101, 115,
                    111, 117, 114, 99, 101, 59, 1, 1, 6, 99, 111, 111, 107, 105, 101, 9, 109, 121,
                    45, 99, 111, 111, 107, 105, 101, 0
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }
}
