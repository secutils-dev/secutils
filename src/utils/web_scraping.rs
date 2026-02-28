mod api_ext;
mod api_trackers;
mod database_ext;
mod page_trackers;

pub use self::{
    api_trackers::{ApiTracker, ApiTrackerConfig, ApiTrackerTarget},
    page_trackers::{PageTracker, PageTrackerConfig, PageTrackerTarget},
};
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
        (UtilsResource::WebScrapingPage, UtilsAction::List) => {
            UtilsActionResult::json(web_scraping.get_page_trackers().await?)
        }
        (UtilsResource::WebScrapingPage, UtilsAction::Create) => UtilsActionResult::json(
            web_scraping
                .create_page_tracker(extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebScrapingPage, UtilsAction::Update { resource_id }) => {
            web_scraping
                .update_page_tracker(resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebScrapingPage, UtilsAction::Delete { resource_id }) => {
            web_scraping.remove_page_tracker(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebScrapingPage,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingPageGetHistory,
            },
        ) => UtilsActionResult::json(
            web_scraping
                .get_page_tracker_history(resource_id, extract_params(params)?)
                .await?,
        ),
        (
            UtilsResource::WebScrapingPage,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingPageClearHistory,
            },
        ) => {
            web_scraping.clear_page_tracker_history(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebScrapingApi, UtilsAction::List) => {
            UtilsActionResult::json(web_scraping.get_api_trackers().await?)
        }
        (UtilsResource::WebScrapingApi, UtilsAction::Create) => UtilsActionResult::json(
            web_scraping
                .create_api_tracker(extract_params(params)?)
                .await?,
        ),
        (UtilsResource::WebScrapingApi, UtilsAction::Update { resource_id }) => {
            web_scraping
                .update_api_tracker(resource_id, extract_params(params)?)
                .await?;
            Ok(UtilsActionResult::empty())
        }
        (UtilsResource::WebScrapingApi, UtilsAction::Delete { resource_id }) => {
            web_scraping.remove_api_tracker(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebScrapingApi,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingApiGetHistory,
            },
        ) => UtilsActionResult::json(
            web_scraping
                .get_api_tracker_history(resource_id, extract_params(params)?)
                .await?,
        ),
        (
            UtilsResource::WebScrapingApi,
            UtilsAction::Execute {
                resource_id: Some(resource_id),
                operation: UtilsResourceOperation::WebScrapingApiClearHistory,
            },
        ) => {
            web_scraping.clear_api_tracker_history(resource_id).await?;
            Ok(UtilsActionResult::empty())
        }
        (
            UtilsResource::WebScrapingApi,
            UtilsAction::Execute {
                resource_id: None,
                operation: UtilsResourceOperation::WebScrapingApiTestRequest,
            },
        ) => UtilsActionResult::json(
            web_scraping
                .test_api_request(extract_params(params)?)
                .await?,
        ),
        _ => Err(SecutilsError::client("Invalid resource or action.").into()),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        retrack::{
            RetrackTracker,
            tags::{
                RETRACK_NOTIFICATIONS_TAG, RETRACK_RESOURCE_ID_TAG, RETRACK_RESOURCE_NAME_TAG,
                RETRACK_RESOURCE_TAG, RETRACK_USER_TAG, prepare_tags,
            },
            tests::{RetrackTrackerValue, mock_retrack_tracker},
        },
        tests::{mock_api_with_config, mock_config, mock_user},
        users::SecretsAccess,
        utils::{
            UtilsAction, UtilsActionParams, UtilsResource, UtilsResourceOperation,
            web_scraping::{
                ApiTracker, ApiTrackerConfig, ApiTrackerTarget as SecutilsApiTrackerTarget,
                PageTracker, PageTrackerConfig, PageTrackerTarget,
                api_ext::{ApiTrackerCreateParams, PageTrackerCreateParams},
                web_scraping_handle_action,
            },
        },
    };
    use httpmock::MockServer;
    use insta::assert_json_snapshot;
    use retrack_types::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        trackers::{
            ApiTarget, PageTarget, TargetRequest, Tracker, TrackerConfig, TrackerCreateParams,
            TrackerDataRevision, TrackerDataValue, TrackerTarget, TrackerUpdateParams,
        },
    };
    use serde_json::json;
    use sqlx::PgPool;
    use std::{slice, time::Duration};
    use time::OffsetDateTime;
    use url::Url;
    use uuid::{Uuid, uuid};

    fn mock_retrack_api_tracker() -> anyhow::Result<Tracker> {
        Ok(Tracker {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            name: "api_one".to_string(),
            enabled: true,
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest::new(Url::parse(
                    "https://api.example.com/data",
                )?)],
                configurator: None,
                extractor: None,
                params: None,
            }),
            job_id: None,
            config: TrackerConfig {
                revisions: 3,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                }),
            },
            tags: vec![],
            actions: vec![],
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            updated_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
    }

    pub struct MockPageTrackerBuilder {
        tracker: PageTracker,
    }

    impl MockPageTrackerBuilder {
        pub fn create<N: Into<String>>(
            id: Uuid,
            name: N,
            retrack: RetrackTracker,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: PageTracker {
                    id,
                    name: name.into(),
                    user_id: mock_user()?.id,
                    retrack,
                    secrets: SecretsAccess::None,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn build(self) -> PageTracker {
            self.tracker
        }
    }

    pub struct MockApiTrackerBuilder {
        tracker: ApiTracker,
    }

    impl MockApiTrackerBuilder {
        pub fn create<N: Into<String>>(
            id: Uuid,
            name: N,
            retrack: RetrackTracker,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: ApiTracker {
                    id,
                    name: name.into(),
                    user_id: mock_user()?.id,
                    retrack,
                    secrets: SecretsAccess::None,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                    updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
                },
            })
        }

        pub fn build(self) -> ApiTracker {
            self.tracker
        }
    }

    #[sqlx::test]
    async fn properly_handles_page_tracker_list_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Vec::<Tracker>::new());
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingPage,
            None,
        )
        .await?;
        assert_json_snapshot!(action_result.into_inner().unwrap(), @"[]");
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::to_value(slice::from_ref(&retrack_tracker)).unwrap());
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(1000),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingPage,
            None,
        )
        .await?;
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&tracker.id.to_string(), "[UUID]");
        settings.add_filter(
            &tracker.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );
        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID]\",\"name\":\"name_one\",\"retrack\":{\"id\":\"00000000-0000-0000-0000-000000000010\",\"enabled\":true,\"config\":{\"revisions\":3,\"job\":{\"schedule\":\"@hourly\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":120000,\"maxAttempts\":5}}},\"target\":{\"type\":\"page\",\"extractor\":\"export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }\"},\"notifications\":false},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}]""###
            );
        });
        retrack_create_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_page_tracker_create_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            // Use partial body match due to a non-deterministic tag with new tracker ID.
            when.method(httpmock::Method::POST)
                .path("/api/trackers")
                .json_body_includes(
                    serde_json::to_string_pretty(&TrackerCreateParams {
                        name: "name_one".to_string(),
                        enabled: true,
                        target: TrackerTarget::Page(PageTarget {
                            extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                            params: None,
                            engine: None,
                            user_agent: None,
                            accept_invalid_certificates: false,
                        }),
                        config: TrackerConfig {
                            revisions: 3,
                            timeout: None,
                            job: Some(SchedulerJobConfig {
                                schedule: "@hourly".to_string(),
                                retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                                    interval: Duration::from_secs(120),
                                    max_attempts: 5,
                                }),
                            }),
                        },
                        tags: prepare_tags(&[
                            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", true),
                            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage)
                        ]),
                        actions: vec![],
                    }).unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::to_value(slice::from_ref(&retrack_tracker)).unwrap());
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebScrapingPage,
            Some(UtilsActionParams::json(json!({
                "name": "name_one",
                "config": {
                    "revisions": 3,
                    "job": Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                "target": {
                    "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                "notifications": true
            }))),
        )
        .await?;

        // Extract tracker to make sure it has been saved.
        let tracker = api
            .web_scraping(&mock_user)
            .get_page_trackers()
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
                @r###""{\"id\":\"[UUID]\",\"name\":\"name_one\",\"retrack\":{\"id\":\"00000000-0000-0000-0000-000000000010\",\"enabled\":true,\"config\":{\"revisions\":3,\"job\":{\"schedule\":\"@hourly\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":120000,\"maxAttempts\":5}}},\"target\":{\"type\":\"page\",\"extractor\":\"export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }\"},\"notifications\":false},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}""###
            );
        });

        retrack_create_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_page_tracker_update_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let updated_retrack_tracker = Tracker {
            name: "name_one_updated".to_string(),
            config: TrackerConfig {
                revisions: 10,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                }),
            },
            target: TrackerTarget::Page(PageTarget {
                extractor: "export async function execute(p) { await p.goto('https://secutils.dev/update'); return await p.content(); }".to_string(),
                engine: None,
                params: None,
                user_agent: None,
                accept_invalid_certificates: false,
            }),
            tags: prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                format!("{RETRACK_RESOURCE_NAME_TAG}:name_one_updated"),
            ]),
            ..retrack_tracker
        };
        let retrack_update_api_mock = retrack_server.mock(|when, then| {
            // Use partial body match due to a non-deterministic tag with new tracker ID.
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    name: Some("name_one_updated".to_string()),
                    config: Some(TrackerConfig {
                        revisions: 10,
                        timeout: None,
                        job: Some(SchedulerJobConfig {
                            schedule: "0 1 * * * *".to_string(),
                            retry_strategy: None,
                        }),
                    }),
                    target: Some(TrackerTarget::Page(PageTarget {
                        extractor: "export async function execute(p) { await p.goto('https://secutils.dev/update'); return await p.content(); }".to_string(),
                        engine: None,
                        params: None,
                        user_agent: None,
                        accept_invalid_certificates: false,
                    })),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_one_updated"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingPage,
            Some(UtilsActionParams::json(json!({
                "name": "name_one_updated",
                "config": {
                    "revisions": 10,
                    "job": Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                "target": {
                    "extractor": "export async function execute(p) { await p.goto('https://secutils.dev/update'); return await p.content(); }".to_string(),
                },
                "notifications": false
            }))),
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let updated_tracker = api
            .web_scraping(&mock_user)
            .get_page_tracker(tracker.id)
            .await?
            .unwrap();
        assert_eq!(
            updated_tracker,
            PageTracker {
                id: tracker.id,
                name: "name_one_updated".to_string(),
                user_id: mock_user.id,
                retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                    id: updated_retrack_tracker.id,
                    enabled: updated_retrack_tracker.enabled,
                    config: updated_retrack_tracker.config,
                    target: updated_retrack_tracker.target,
                    notifications: false,
                })),
                secrets: SecretsAccess::None,
                created_at: tracker.created_at,
                updated_at: updated_tracker.updated_at
            }
        );
        retrack_update_api_mock.assert();
        retrack_get_api_mock.assert_calls(2);

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_page_tracker_delete_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingPage,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        // Extract tracker to make sure it has been updated.
        let deleted_tracker = web_scraping.get_page_tracker(tracker.id).await?;
        assert!(deleted_tracker.is_none());

        retrack_get_api_mock.assert();
        retrack_delete_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_get_page_tracker_history_operation(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id))
                .query_param("calculateDiff", "false");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[TrackerDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000100"),
                    tracker_id: retrack_tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800).unwrap(),
                    data: TrackerDataValue::new(json!({ "one": 1 })),
                }]);
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(tracker.id),
                operation: UtilsResourceOperation::WebScrapingPageGetHistory,
            },
            UtilsResource::WebScrapingPage,
            Some(UtilsActionParams::json(json!({
                "refresh": false
            }))),
        )
        .await?;

        assert_json_snapshot!(
            serde_json::to_string(&action_result.into_inner().unwrap())?,
            @r###""[{\"id\":\"00000000-0000-0000-0000-000000000100\",\"trackerId\":\"00000000-0000-0000-0000-000000000010\",\"data\":{\"original\":{\"one\":1}},\"createdAt\":946720800}]""###
        );

        retrack_get_api_mock.assert();
        retrack_list_revisions_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_clear_page_tracker_history_operation(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_clear_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(204).header("Content-Type", "application/json");
        });

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(tracker.id),
                operation: UtilsResourceOperation::WebScrapingPageClearHistory,
            },
            UtilsResource::WebScrapingPage,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());
        retrack_get_api_mock.assert();
        retrack_clear_revisions_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_api_tracker_list_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Vec::<Tracker>::new());
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingApi,
            None,
        )
        .await?;
        assert_json_snapshot!(action_result.into_inner().unwrap(), @"[]");
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::to_value(slice::from_ref(&retrack_tracker)).unwrap());
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(1000),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: SecutilsApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::List,
            UtilsResource::WebScrapingApi,
            None,
        )
        .await?;
        let mut settings = insta::Settings::clone_current();
        settings.add_filter(&tracker.id.to_string(), "[UUID]");
        settings.add_filter(
            &tracker.created_at.unix_timestamp().to_string(),
            "[TIMESTAMP]",
        );
        settings.bind(|| {
            assert_json_snapshot!(
                serde_json::to_string(&action_result.into_inner().unwrap()).unwrap(),
                @r###""[{\"id\":\"[UUID]\",\"name\":\"api_one\",\"retrack\":{\"id\":\"00000000-0000-0000-0000-000000000010\",\"enabled\":true,\"config\":{\"revisions\":3,\"job\":{\"schedule\":\"@hourly\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":120000,\"maxAttempts\":5}}},\"target\":{\"url\":\"https://api.example.com/data\"},\"notifications\":false},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}]""###
            );
        });
        retrack_create_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_api_tracker_create_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers")
                .json_body_includes(
                    serde_json::to_string_pretty(&TrackerCreateParams {
                        name: "api_one".to_string(),
                        enabled: true,
                        target: TrackerTarget::Api(ApiTarget {
                            requests: vec![TargetRequest::new(
                                Url::parse("https://api.example.com/data").unwrap(),
                            )],
                            configurator: None,
                            extractor: None,
                            params: None,
                        }),
                        config: TrackerConfig {
                            revisions: 3,
                            timeout: None,
                            job: Some(SchedulerJobConfig {
                                schedule: "@hourly".to_string(),
                                retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                                    interval: Duration::from_secs(120),
                                    max_attempts: 5,
                                }),
                            }),
                        },
                        tags: prepare_tags(&[
                            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", true),
                            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        ]),
                        actions: vec![],
                    })
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::to_value(slice::from_ref(&retrack_tracker)).unwrap());
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Create,
            UtilsResource::WebScrapingApi,
            Some(UtilsActionParams::json(json!({
                "name": "api_one",
                "config": {
                    "revisions": 3,
                    "job": Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                "target": {
                    "url": "https://api.example.com/data",
                },
                "notifications": true
            }))),
        )
        .await?;

        let tracker = api
            .web_scraping(&mock_user)
            .get_api_trackers()
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
                @r###""{\"id\":\"[UUID]\",\"name\":\"api_one\",\"retrack\":{\"id\":\"00000000-0000-0000-0000-000000000010\",\"enabled\":true,\"config\":{\"revisions\":3,\"job\":{\"schedule\":\"@hourly\",\"retryStrategy\":{\"type\":\"constant\",\"interval\":120000,\"maxAttempts\":5}}},\"target\":{\"url\":\"https://api.example.com/data\"},\"notifications\":false},\"createdAt\":[TIMESTAMP],\"updatedAt\":[TIMESTAMP]}""###
            );
        });

        retrack_create_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_api_tracker_update_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: SecutilsApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let updated_retrack_tracker = Tracker {
            name: "api_one_updated".to_string(),
            config: TrackerConfig {
                revisions: 10,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "0 1 * * * *".to_string(),
                    retry_strategy: None,
                }),
            },
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest::new(Url::parse(
                    "https://api.example.com/updated",
                )?)],
                configurator: None,
                extractor: None,
                params: None,
            }),
            tags: prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                format!("{RETRACK_RESOURCE_NAME_TAG}:api_one_updated"),
            ]),
            ..retrack_tracker
        };
        let retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    name: Some("api_one_updated".to_string()),
                    config: Some(TrackerConfig {
                        revisions: 10,
                        timeout: None,
                        job: Some(SchedulerJobConfig {
                            schedule: "0 1 * * * *".to_string(),
                            retry_strategy: None,
                        }),
                    }),
                    target: Some(TrackerTarget::Api(ApiTarget {
                        requests: vec![TargetRequest::new(
                            Url::parse("https://api.example.com/updated").unwrap(),
                        )],
                        configurator: None,
                        extractor: None,
                        params: None,
                    })),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:api_one_updated"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Update {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingApi,
            Some(UtilsActionParams::json(json!({
                "name": "api_one_updated",
                "config": {
                    "revisions": 10,
                    "job": Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                "target": {
                    "url": "https://api.example.com/updated",
                },
                "notifications": false
            }))),
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        let updated_tracker = api
            .web_scraping(&mock_user)
            .get_api_tracker(tracker.id)
            .await?
            .unwrap();
        assert_eq!(
            updated_tracker,
            ApiTracker {
                id: tracker.id,
                name: "api_one_updated".to_string(),
                user_id: mock_user.id,
                retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                    id: updated_retrack_tracker.id,
                    enabled: updated_retrack_tracker.enabled,
                    config: updated_retrack_tracker.config,
                    target: updated_retrack_tracker.target,
                    notifications: false,
                })),
                secrets: SecretsAccess::None,
                created_at: tracker.created_at,
                updated_at: updated_tracker.updated_at
            }
        );
        retrack_update_api_mock.assert();
        retrack_get_api_mock.assert_calls(2);

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_api_tracker_delete_operation(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: SecutilsApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Delete {
                resource_id: tracker.id,
            },
            UtilsResource::WebScrapingApi,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());

        let deleted_tracker = web_scraping.get_api_tracker(tracker.id).await?;
        assert!(deleted_tracker.is_none());

        retrack_get_api_mock.assert();
        retrack_delete_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_get_api_tracker_history_operation(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: SecutilsApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_list_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id))
                .query_param("calculateDiff", "false");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[TrackerDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000100"),
                    tracker_id: retrack_tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800).unwrap(),
                    data: TrackerDataValue::new(json!({ "key": "value" })),
                }]);
        });
        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(tracker.id),
                operation: UtilsResourceOperation::WebScrapingApiGetHistory,
            },
            UtilsResource::WebScrapingApi,
            Some(UtilsActionParams::json(json!({
                "refresh": false
            }))),
        )
        .await?;

        assert_json_snapshot!(
            serde_json::to_string(&action_result.into_inner().unwrap())?,
            @r###""[{\"id\":\"00000000-0000-0000-0000-000000000100\",\"trackerId\":\"00000000-0000-0000-0000-000000000010\",\"data\":{\"original\":{\"key\":\"value\"}},\"createdAt\":946720800}]""###
        );

        retrack_get_api_mock.assert();
        retrack_list_revisions_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_handles_clear_api_tracker_history_operation(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_retrack_api_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "api_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: None,
                    }),
                },
                target: SecutilsApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: true,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_clear_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(204).header("Content-Type", "application/json");
        });

        let action_result = web_scraping_handle_action(
            mock_user.clone(),
            &api,
            UtilsAction::Execute {
                resource_id: Some(tracker.id),
                operation: UtilsResourceOperation::WebScrapingApiClearHistory,
            },
            UtilsResource::WebScrapingApi,
            None,
        )
        .await?;
        assert!(action_result.into_inner().is_none());
        retrack_get_api_mock.assert();
        retrack_clear_revisions_api_mock.assert();

        Ok(())
    }
}
