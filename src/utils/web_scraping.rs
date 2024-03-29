mod api_ext;
mod database_ext;
mod web_page_trackers;

pub use self::web_page_trackers::{
    web_page_content_revisions_diff, web_page_resources_revisions_diff, WebPageContentTrackerTag,
    WebPageDataRevision, WebPageResource, WebPageResourceContent, WebPageResourceContentData,
    WebPageResourceDiffStatus, WebPageResourcesData, WebPageResourcesTrackerTag, WebPageTracker,
    WebPageTrackerKind, WebPageTrackerSettings, WebPageTrackerTag, WebScraperContentRequest,
    WebScraperContentRequestScripts, WebScraperContentResponse, WebScraperErrorResponse,
    WebScraperResource, WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
    WebScraperResourcesResponse,
};
use self::web_page_trackers::{WebPageResourceInternal, WebPageResourcesTrackerInternalTag};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{
        UtilsAction, UtilsActionParams, UtilsActionResult, UtilsResource, UtilsResourceOperation,
    },
};
use serde::Deserialize;

fn extract_params<T: for<'de> Deserialize<'de>>(
    params: Option<UtilsActionParams>,
) -> anyhow::Result<T> {
    params
        .ok_or_else(|| SecutilsError::client("Missing required action parameters."))?
        .into_inner()
}

pub async fn web_scraping_handle_action<DR: DnsResolver, ET: EmailTransport>(
    user: User,
    api: &Api<DR, ET>,
    action: UtilsAction,
    resource: UtilsResource,
    params: Option<UtilsActionParams>,
) -> anyhow::Result<UtilsActionResult> {
    let web_scraping = api.web_scraping(&user);
    match (resource, action) {
        (UtilsResource::WebScrapingResources, UtilsAction::List) => {
            UtilsActionResult::json(web_scraping.get_resources_trackers().await?)
        }
        (UtilsResource::WebScrapingContent, UtilsAction::List) => {
            UtilsActionResult::json(web_scraping.get_content_trackers().await?)
        }
        (UtilsResource::WebScrapingResources, UtilsAction::Create) => UtilsActionResult::json(
            web_scraping
                .create_resources_tracker(extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebScrapingContent, UtilsAction::Create) => UtilsActionResult::json(
            web_scraping
                .create_content_tracker(extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebScrapingResources, UtilsAction::Update { resource_id }) => {
            web_scraping
                .update_resources_tracker(resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebScrapingContent, UtilsAction::Update { resource_id }) => {
            web_scraping
                .update_content_tracker(resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebScrapingResources | UtilsResource::WebScrapingContent,
            UtilsAction::Delete { resource_id },
        ) => {
            web_scraping.remove_web_page_tracker(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebScrapingResources,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebScrapingGetHistory,
            },
        ) => UtilsActionResult::json(
            web_scraping
                .get_resources_tracker_history(resource_id, extract_params(params)?)
                .await?,
        ),
        (
            UtilsResource::WebScrapingContent,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebScrapingGetHistory,
            },
        ) => UtilsActionResult::json(
            web_scraping
                .get_content_tracker_history(resource_id, extract_params(params)?)
                .await?,
        ),
        (
            UtilsResource::WebScrapingResources | UtilsResource::WebScrapingContent,
            UtilsAction::Execute {
                resource_id,
                operation: UtilsResourceOperation::WebScrapingClearHistory,
            },
        ) => {
            web_scraping
                .clear_web_page_tracker_history(resource_id)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    pub use crate::utils::web_scraping::api_ext::{
        WebPageTrackerCreateParams, WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME,
        WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
    };
    use crate::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        tests::{mock_api, mock_user},
        utils::{
            web_scraping::{
                api_ext::{
                    WebPageContentTrackerGetHistoryParams, WebPageResourcesTrackerGetHistoryParams,
                },
                web_scraping_handle_action, WebPageContentTrackerTag, WebPageDataRevision,
                WebPageResourceInternal, WebPageResourcesData, WebPageResourcesTrackerInternalTag,
                WebPageTracker, WebPageTrackerSettings, WebPageTrackerTag,
            },
            UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
        },
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use sqlx::PgPool;
    use std::{collections::HashMap, time::Duration};
    use time::OffsetDateTime;
    use url::Url;
    use uuid::{uuid, Uuid};

    pub struct MockWebPageTrackerBuilder<Tag: WebPageTrackerTag> {
        tracker: WebPageTracker<Tag>,
    }

    impl<Tag: WebPageTrackerTag> MockWebPageTrackerBuilder<Tag> {
        pub fn create<N: Into<String>>(
            id: Uuid,
            name: N,
            url: &str,
            revisions: usize,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: WebPageTracker {
                    id,
                    name: name.into(),
                    user_id: mock_user()?.id,
                    job_id: None,
                    job_config: None,
                    url: Url::parse(url)?,
                    settings: WebPageTrackerSettings {
                        revisions,
                        delay: Duration::from_millis(2000),
                        scripts: Default::default(),
                        headers: Default::default(),
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    meta: None,
                },
            })
        }

        pub fn with_schedule<S: Into<String>>(mut self, schedule: S) -> Self {
            self.tracker.job_config = Some(SchedulerJobConfig {
                schedule: schedule.into(),
                retry_strategy: None,
                notifications: false,
            });
            self
        }

        pub fn with_job_config(mut self, job_config: SchedulerJobConfig) -> Self {
            self.tracker.job_config = Some(job_config);
            self
        }

        pub fn with_job_id(mut self, job_id: Uuid) -> Self {
            self.tracker.job_id = Some(job_id);
            self
        }

        pub fn with_delay_millis(mut self, millis: u64) -> Self {
            self.tracker.settings.delay = Duration::from_millis(millis);
            self
        }

        pub fn with_scripts(mut self, scripts: HashMap<String, String>) -> Self {
            self.tracker.settings.scripts = Some(scripts);
            self
        }

        pub fn build(self) -> WebPageTracker<Tag> {
            self.tracker
        }
    }

    #[sqlx::test]
    async fn properly_handles_resources_list_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingResources,
            None,
        )
        .await?;
        assert_json_snapshot!(action_result.into_inner().unwrap(), @"[]");

        let tracker_one = api
            .web_scraping(&mock_user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        let tracker_two = api
            .web_scraping(&mock_user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingResources,
            None,
        )
        .await?;
        let mut settings = insta::Settings::clone_current();
        for tracker in [tracker_one, tracker_two] {
            settings.add_filter(&tracker.id.to_string(), "[UUID]");
            settings.add_filter(
                &tracker.created_at.unix_timestamp().to_string(),
                "[TIMESTAMP]",
            );
        }
        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID]\",\"name\":\"name_one\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]},{\"id\":\"[UUID]\",\"name\":\"name_two\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]}]""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_content_list_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingContent,
            None,
        )
        .await?;
        assert_json_snapshot!(action_result.into_inner().unwrap(), @"[]");

        let tracker_one = api
            .web_scraping(&mock_user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(1000),
                        max_attempts: 5,
                    }),
                    notifications: true,
                }),
            })
            .await?;
        let tracker_two = api
            .web_scraping(&mock_user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingContent,
            None,
        )
        .await?;
        let mut settings = insta::Settings::clone_current();
        for tracker in [tracker_one, tracker_two] {
            settings.add_filter(&tracker.id.to_string(), "[UUID]");
            settings.add_filter(
                &tracker.created_at.unix_timestamp().to_string(),
                "[TIMESTAMP]",
            );
        }
        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID]\",\"name\":\"name_one\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":1000000,\"maxAttempts\":5},\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]},{\"id\":\"[UUID]\",\"name\":\"name_two\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":1000000,\"maxAttempts\":5},\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]}]""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_resources_create_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebScrapingResources,
            Some(UtilsActionParams::json(json!({
                "name": "name_one",
                "url": "https://secutils.dev",
                "settings": WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                "jobConfig": SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(1),
                        max_interval: Duration::from_secs(200),
                        max_attempts: 10,
                    }),
                    notifications: true,
                }
            }))),
        )
        .await?;

        // Extract tracker to make sure it has been saved.
        let tracker = api
            .web_scraping(&mock_user)
            .get_resources_trackers()
            .await?
            .pop()
            .unwrap();
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&tracker.id.to_string(), "[UUID]");
        settings.add_filter(
            &tracker.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""{\"id\":\"[UUID]\",\"name\":\"name_one\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"retryStrategy\":{\"type\":\"linear\",\"initialInterval\":120000,\"increment\":1000,\"maxInterval\":200000,\"maxAttempts\":10},\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]}""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_content_create_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebScrapingContent,
            Some(UtilsActionParams::json(json!({
                "name": "name_one",
                "url": "https://secutils.dev",
                "settings": WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                 "jobConfig": SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                     retry_strategy: Some(SchedulerJobRetryStrategy::Exponential {
                        initial_interval: Duration::from_secs(120),
                        multiplier: 2,
                        max_interval: Duration::from_secs(200),
                        max_attempts: 10,
                    }),
                    notifications: true,
                }
            }))),
        )
        .await?;

        // Extract tracker to make sure it has been saved.
        let tracker = api
            .web_scraping(&mock_user)
            .get_content_trackers()
            .await?
            .pop()
            .unwrap();
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&tracker.id.to_string(), "[UUID]");
        settings.add_filter(
            &tracker.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );

        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""{\"id\":\"[UUID]\",\"name\":\"name_one\",\"url\":\"https://secutils.dev/\",\"jobConfig\":{\"schedule\":\"0 0 * * * *\",\"retryStrategy\":{\"type\":\"exponential\",\"initialInterval\":120000,\"multiplier\":2,\"maxInterval\":200000,\"maxAttempts\":10},\"notifications\":true},\"settings\":{\"revisions\":3,\"delay\":2000},\"createdAt\":[TIMESTAMP]}""###
            );
        });

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_resources_update_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let tracker = api
            .web_scraping(&mock_user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingResources,
            Some(UtilsActionParams::json(json!({
                "name": "name_one_updated",
                "url": "https://secutils.dev/update",
                "settings": WebPageTrackerSettings {
                    revisions: 10,
                    delay: Duration::from_millis(3000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                "jobConfig": SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }
            }))),
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let updated_tracker = api
            .web_scraping(&mock_user)
            .get_resources_tracker(tracker.id)
            .await?
            .unwrap();
        assert_eq!(
            updated_tracker,
            WebPageTracker {
                id: tracker.id,
                name: "name_one_updated".to_string(),
                url: "https://secutils.dev/update".parse()?,
                user_id: mock_user.id,
                job_id: None,
                settings: WebPageTrackerSettings {
                    revisions: 10,
                    delay: Duration::from_millis(3000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
                created_at: tracker.created_at,
                meta: None
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_content_update_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let tracker = api
            .web_scraping(&mock_user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingContent,
            Some(UtilsActionParams::json(json!({
                "name": "name_one_updated",
                "url": "https://secutils.dev/update",
                "settings": WebPageTrackerSettings {
                    revisions: 10,
                    delay: Duration::from_millis(3000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                "jobConfig": SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                },
            }))),
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let updated_tracker = api
            .web_scraping(&mock_user)
            .get_content_tracker(tracker.id)
            .await?
            .unwrap();
        assert_eq!(
            updated_tracker,
            WebPageTracker {
                id: tracker.id,
                name: "name_one_updated".to_string(),
                url: "https://secutils.dev/update".parse()?,
                user_id: mock_user.id,
                job_id: None,
                settings: WebPageTrackerSettings {
                    revisions: 10,
                    delay: Duration::from_millis(3000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
                created_at: tracker.created_at,
                meta: None
            }
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_resources_delete_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let tracker = api
            .web_scraping(&mock_user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingResources,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let deleted_tracker = api
            .web_scraping(&mock_user)
            .get_resources_tracker(tracker.id)
            .await?;
        assert!(deleted_tracker.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_content_delete_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let tracker = api
            .web_scraping(&mock_user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingContent,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let deleted_tracker = api
            .web_scraping(&mock_user)
            .get_content_tracker(tracker.id)
            .await?;
        assert!(deleted_tracker.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_get_history_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert trackers and history.
        let resources_tracker = api
            .web_scraping(&mock_user)
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        let content_tracker = api
            .web_scraping(&mock_user)
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerInternalTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: resources_tracker.id,
                    data: WebPageResourcesData {
                        scripts: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/script_one.js")?),
                            content: None,
                        }],
                        styles: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/style_one.css")?),
                            content: None,
                        }],
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerInternalTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    tracker_id: resources_tracker.id,
                    data: WebPageResourcesData {
                        scripts: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/script_two.js")?),
                            content: None,
                        }],
                        styles: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/style_two.css")?),
                            content: None,
                        }],
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720900)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000003"),
                    tracker_id: content_tracker.id,
                    data: "some-data".to_string(),
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000004"),
                    tracker_id: content_tracker.id,
                    data: "other-data".to_string(),
                    created_at: OffsetDateTime::from_unix_timestamp(946720900)?,
                },
            )
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: resources_tracker.id,
                operation: UtilsResourceOperation::WebScrapingGetHistory,
            },
            UtilsResource::WebScrapingResources,
            Some(UtilsActionParams::json(json!({
                "refresh": false,
                "calculateDiff": true
            }))),
        )
        .await?;

        assert_json_snapshot!(
            serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
            @r###""[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"data\":{\"scripts\":[{\"url\":\"http://localhost:1234/script_one.js\"}],\"styles\":[{\"url\":\"http://localhost:1234/style_one.css\"}]},\"createdAt\":946720800},{\"id\":\"00000000-0000-0000-0000-000000000002\",\"data\":{\"scripts\":[{\"url\":\"http://localhost:1234/script_two.js\",\"diffStatus\":\"added\"},{\"url\":\"http://localhost:1234/script_one.js\",\"diffStatus\":\"removed\"}],\"styles\":[{\"url\":\"http://localhost:1234/style_two.css\",\"diffStatus\":\"added\"},{\"url\":\"http://localhost:1234/style_one.css\",\"diffStatus\":\"removed\"}]},\"createdAt\":946720900}]""###
        );

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: content_tracker.id,
                operation: UtilsResourceOperation::WebScrapingGetHistory,
            },
            UtilsResource::WebScrapingContent,
            Some(UtilsActionParams::json(json!({
                "refresh": false
            }))),
        )
        .await?;

        assert_json_snapshot!(
            serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
            @r###""[{\"id\":\"00000000-0000-0000-0000-000000000003\",\"data\":\"some-data\",\"createdAt\":946720800},{\"id\":\"00000000-0000-0000-0000-000000000004\",\"data\":\"other-data\",\"createdAt\":946720900}]""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_clear_history_operation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        // Insert tracker and history.
        let web_scraping = api.web_scraping(&mock_user);
        let resources_tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        let content_tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 0 * * * *".to_string(),
                    retry_strategy: None,
                    notifications: true,
                }),
            })
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerInternalTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_id: resources_tracker.id,
                    data: WebPageResourcesData {
                        scripts: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/script_one.js")?),
                            content: None,
                        }],
                        styles: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/style_one.css")?),
                            content: None,
                        }],
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerInternalTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000002"),
                    tracker_id: resources_tracker.id,
                    data: WebPageResourcesData {
                        scripts: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/script_two.js")?),
                            content: None,
                        }],
                        styles: vec![WebPageResourceInternal {
                            url: Some(Url::parse("http://localhost:1234/style_two.css")?),
                            content: None,
                        }],
                    },
                    created_at: OffsetDateTime::from_unix_timestamp(946720900)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000003"),
                    tracker_id: content_tracker.id,
                    data: "some-data".to_string(),
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                },
            )
            .await?;
        api.db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(
                &WebPageDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000004"),
                    tracker_id: content_tracker.id,
                    data: "some-other-data".to_string(),
                    created_at: OffsetDateTime::from_unix_timestamp(946720900)?,
                },
            )
            .await?;

        assert_eq!(
            api.web_scraping(&mock_user)
                .get_resources_tracker_history(
                    resources_tracker.id,
                    WebPageResourcesTrackerGetHistoryParams {
                        refresh: false,
                        calculate_diff: false,
                    }
                )
                .await?
                .len(),
            2
        );
        assert_eq!(
            api.web_scraping(&mock_user)
                .get_content_tracker_history(
                    content_tracker.id,
                    WebPageContentTrackerGetHistoryParams {
                        refresh: false,
                        calculate_diff: false
                    }
                )
                .await?
                .len(),
            2
        );

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: resources_tracker.id,
                operation: UtilsResourceOperation::WebScrapingClearHistory,
            },
            UtilsResource::WebScrapingResources,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: content_tracker.id,
                operation: UtilsResourceOperation::WebScrapingClearHistory,
            },
            UtilsResource::WebScrapingContent,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        assert!(api
            .web_scraping(&mock_user)
            .get_resources_tracker_history(
                resources_tracker.id,
                WebPageResourcesTrackerGetHistoryParams {
                    refresh: false,
                    calculate_diff: false,
                }
            )
            .await?
            .is_empty());
        assert!(api
            .web_scraping(&mock_user)
            .get_content_tracker_history(
                content_tracker.id,
                WebPageContentTrackerGetHistoryParams {
                    refresh: false,
                    calculate_diff: false
                }
            )
            .await?
            .is_empty());

        Ok(())
    }
}
