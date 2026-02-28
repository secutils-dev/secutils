mod api_tracker_create_params;
mod api_tracker_test_params;
mod api_tracker_update_params;
mod page_tracker_create_params;
mod page_tracker_get_history_params;
mod page_tracker_update_params;

pub use self::{
    api_tracker_create_params::ApiTrackerCreateParams,
    api_tracker_test_params::{ApiTrackerTestParams, ApiTrackerTestResult},
    api_tracker_update_params::ApiTrackerUpdateParams,
    page_tracker_create_params::PageTrackerCreateParams,
    page_tracker_get_history_params::PageTrackerGetHistoryParams,
    page_tracker_update_params::PageTrackerUpdateParams,
};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    retrack::{
        RetrackTracker,
        tags::{
            RETRACK_NOTIFICATIONS_TAG, RETRACK_RESOURCE_ID_TAG, RETRACK_RESOURCE_NAME_TAG,
            RETRACK_RESOURCE_TAG, RETRACK_USER_TAG, get_tag_value, prepare_tags,
        },
    },
    scheduler::CronExt,
    users::User,
    utils::{
        UtilsResource,
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_scraping::{
            ApiTracker, ApiTrackerConfig, ApiTrackerTarget, PageTracker, PageTrackerConfig,
            PageTrackerTarget,
        },
    },
};
use anyhow::{anyhow, bail};
use croner::Cron;
use http::Method;
use retrack_types::trackers::{
    ApiTarget, PageTarget, TargetRequest, TrackerConfig, TrackerCreateParams, TrackerDataRevision,
    TrackerTarget, TrackerUpdateParams,
};
use std::{collections::HashSet, time::Duration};
use time::OffsetDateTime;
use tracing::error;
use uuid::Uuid;

/// We currently support up to 10 retry attempts for the web page tracker.
const MAX_PAGE_TRACKER_RETRY_ATTEMPTS: u32 = 10;

/// We currently support a minimum 60 seconds between retry attempts for the web page tracker.
const MIN_PAGE_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// We currently support the maximum 12 hours between retry attempts for the web page tracker.
const MAX_PAGE_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(12 * 3600);

/// We currently support up to 10 retry attempts for the API tracker.
const MAX_API_TRACKER_RETRY_ATTEMPTS: u32 = 10;

/// We currently support a minimum 60 seconds between retry attempts for the API tracker.
const MIN_API_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// We currently support the maximum 12 hours between retry attempts for the API tracker.
const MAX_API_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(12 * 3600);

pub struct WebScrapingApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> WebScrapingApiExt<'a, 'u, DR, ET> {
    /// Creates WebScraping API.
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Returns all page trackers.
    pub async fn get_page_trackers(&self) -> anyhow::Result<Vec<PageTracker>> {
        // Fetch trackers from the database and Retrack.
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let retrack = self.api.retrack();
        let utils_resource = UtilsResource::WebScrapingPage;
        let tags = [
            format!("{RETRACK_USER_TAG}:{}", self.user.id),
            format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
        ];
        let (mut trackers, retrack_trackers) = tokio::try_join!(
            web_scraping.get_page_trackers(),
            retrack.list_trackers(&tags)
        )?;

        // Enhance trackers with Retrack data.
        let (resource, resource_group) = utils_resource.into();
        let mut retrack_trackers_map = retrack_trackers
            .into_iter()
            .map(|tracker| (tracker.id, tracker))
            .collect::<std::collections::HashMap<_, _>>();
        for tracker in trackers.iter_mut() {
            if let Some(retrack_tracker) = retrack_trackers_map.remove(&tracker.retrack.id()) {
                tracker.retrack = RetrackTracker::from_value(retrack_tracker);
            } else {
                error!(
                    user.id = %self.user.id,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    retrack.id = %tracker.retrack.id(),
                    "Page tracker is not found in Retrack."
                );
            }
        }

        // Iterate through retrack trackers that aren't in the database and them to the error log.
        for retrack_tracker in retrack_trackers_map.values() {
            error!(
                user.id = %self.user.id,
                util.resource_id = get_tag_value(&retrack_tracker.tags, RETRACK_RESOURCE_ID_TAG),
                util.resource_name = retrack_tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %retrack_tracker.id,
                "Found a dangling Retrack tracker that needs to be removed."
            );
        }

        Ok(trackers)
    }

    /// Returns a page tracker by its ID.
    pub async fn get_page_tracker(&self, id: Uuid) -> anyhow::Result<Option<PageTracker>> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let tracker = if let Some(mut tracker) = web_scraping.get_page_tracker(id).await? {
            if let Some(retrack_tracker) =
                self.api.retrack().get_tracker(tracker.retrack.id()).await?
            {
                tracker.retrack = RetrackTracker::from_value(retrack_tracker);
            } else {
                let (resource, resource_group) = UtilsResource::WebScrapingPage.into();
                error!(
                    user.id = %self.user.id,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    retrack.id = %tracker.retrack.id(),
                    "Page tracker is not found in Retrack."
                );
            }

            Some(tracker)
        } else {
            None
        };

        Ok(tracker)
    }

    /// Creates a new page tracker.
    pub async fn create_page_tracker(
        &self,
        params: PageTrackerCreateParams,
    ) -> anyhow::Result<PageTracker> {
        // 1. Perform validation.
        self.validate_page_tracker_name(&params.name)?;
        self.validate_page_tracker_config(&params.config)?;
        self.validate_page_tracker_target(&params.target)?;

        // 2. Create a new Retrack tracker.
        let id = Uuid::now_v7();
        let retrack = self.api.retrack();
        let utils_resource = UtilsResource::WebScrapingPage;
        let retrack_tracker = retrack
            .create_tracker(&TrackerCreateParams {
                enabled: params.enabled,
                name: params.name.clone(),
                target: TrackerTarget::Page(PageTarget {
                    extractor: params.target.extractor,
                    params: if !params.secrets.is_none() {
                        let secrets = self
                            .api
                            .secrets(self.user)
                            .get_decrypted_secrets(&params.secrets)
                            .await
                            .unwrap_or_default();
                        if secrets.is_empty() {
                            None
                        } else {
                            Some(serde_json::json!({ "secrets": secrets }))
                        }
                    } else {
                        None
                    },
                    engine: None,
                    user_agent: None,
                    accept_invalid_certificates: false,
                }),
                config: TrackerConfig {
                    revisions: params.config.revisions,
                    timeout: None,
                    job: params.config.job,
                },
                tags: prepare_tags(&[
                    format!("{RETRACK_USER_TAG}:{}", self.user.id),
                    format!("{RETRACK_NOTIFICATIONS_TAG}:{}", params.notifications),
                    format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
                    format!("{RETRACK_RESOURCE_ID_TAG}:{id}"),
                    format!("{RETRACK_RESOURCE_NAME_TAG}:{}", params.name),
                ]),
                actions: vec![],
            })
            .await?;

        // Preserve timestamp only up to seconds.
        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;

        // 3. Create a new page tracker in the database.
        let tracker = PageTracker {
            id,
            name: params.name,
            user_id: self.user.id,
            retrack: RetrackTracker::from_value(retrack_tracker),
            secrets: params.secrets.clone(),
            created_at,
            updated_at: created_at,
        };

        let web_scraping = self.api.db.web_scraping(self.user.id);
        if let Err(err) = web_scraping.insert_page_tracker(&tracker).await {
            // If the tracker creation failed, remove it from Retrack.
            if let Err(err) = retrack.remove_tracker(tracker.retrack.id()).await {
                let (resource, resource_group) = utils_resource.into();
                error!(
                    util.resource = resource,
                    util.resource_group = resource_group,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    retrack.id = %tracker.retrack.id(),
                    "Failed to remove tracker from Retrack: {err:?}"
                );
            }

            return Err(err);
        }

        Ok(tracker)
    }

    /// Updates existing page tracker.
    pub async fn update_page_tracker(
        &self,
        id: Uuid,
        params: PageTrackerUpdateParams,
    ) -> anyhow::Result<PageTracker> {
        let utils_resource = UtilsResource::WebScrapingPage;
        let (resource, resource_group) = utils_resource.into();
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let Some(existing_tracker) = web_scraping.get_page_tracker(id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %id,
                util.resource = resource,
                util.resource_group = resource_group,
                "Page tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "Page tracker ('{id}') is not found."
            )));
        };

        // 1. Perform validation.
        if let Some(ref name) = params.name {
            self.validate_page_tracker_name(name)?;
        }
        if let Some(ref config) = params.config {
            self.validate_page_tracker_config(config)?;
        }
        if let Some(ref target) = params.target {
            self.validate_page_tracker_target(target)?;
        }

        // 2. Retrieve the existing tracker from Retrack.
        let retrack = self.api.retrack();
        let Some(retrack_tracker) = retrack.get_tracker(existing_tracker.retrack.id()).await?
        else {
            error!(
                user.id = %existing_tracker.user_id,
                util.resource_id = %existing_tracker.id,
                util.resource_name = existing_tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %existing_tracker.retrack.id(),
                "Page tracker is not found in Retrack."
            );
            bail!(SecutilsError::client(format!(
                "Page tracker ('{id}') is not found in Retrack."
            )));
        };

        // 3. Update tracker in Retrack.
        let effective_secrets = params.secrets.as_ref().unwrap_or(&existing_tracker.secrets);
        let page_params = if !effective_secrets.is_none() {
            let secrets = self
                .api
                .secrets(self.user)
                .get_decrypted_secrets(effective_secrets)
                .await
                .unwrap_or_default();
            if secrets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "secrets": secrets }))
            }
        } else {
            None
        };
        let retrack_tracker = retrack
            .update_tracker(
                retrack_tracker.id,
                &TrackerUpdateParams {
                    name: params.name.clone(),
                    enabled: params.enabled,
                    config: params.config.map(|config| TrackerConfig {
                        revisions: config.revisions,
                        timeout: None,
                        job: config.job,
                    }),
                    target: if let Some(target) = params.target {
                        Some(TrackerTarget::Page(PageTarget {
                            extractor: target.extractor,
                            params: page_params,
                            engine: None,
                            user_agent: None,
                            accept_invalid_certificates: false,
                        }))
                    } else if params.secrets.is_some() {
                        Some(match retrack_tracker.target {
                            TrackerTarget::Page(page) => TrackerTarget::Page(PageTarget {
                                params: page_params,
                                ..page
                            }),
                            other => other,
                        })
                    } else {
                        None
                    },
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", self.user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", params.notifications),
                        format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{id}"),
                        format!(
                            "{RETRACK_RESOURCE_NAME_TAG}:{}",
                            params.name.as_ref().unwrap_or(&existing_tracker.name)
                        ),
                    ])),
                    ..Default::default()
                },
            )
            .await?;

        let tracker = PageTracker {
            name: params.name.unwrap_or(existing_tracker.name),
            retrack: RetrackTracker::from_value(retrack_tracker),
            secrets: params
                .secrets
                .clone()
                .unwrap_or(existing_tracker.secrets.clone()),
            // Preserve timestamp only up to seconds.
            updated_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            ..existing_tracker
        };

        web_scraping.update_page_tracker(&tracker).await?;

        Ok(tracker)
    }

    /// Removes existing page tracker and all history.
    pub async fn remove_page_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let (resource, resource_group) = UtilsResource::WebScrapingPage.into();

        // 1. Retrieve the existing tracker from the database.
        let Some(tracker) = web_scraping.get_page_tracker(id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %id,
                util.resource = resource,
                util.resource_group = resource_group,
                "Page tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "Page tracker ('{id}') is not found."
            )));
        };

        // 2. Retrieve the existing tracker from Retrack.
        let retrack = self.api.retrack();
        if let Some(retrack_tracker) = retrack.get_tracker(tracker.retrack.id()).await? {
            retrack.remove_tracker(retrack_tracker.id).await?;
        } else {
            error!(
                user.id = %tracker.user_id,
                util.resource_id = %tracker.id,
                util.resource_name = tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %tracker.retrack.id(),
                "Page tracker is not found in Retrack, removing will be skipped."
            );
        };

        web_scraping.remove_page_tracker(id).await
    }

    /// Persists history for the specified page tracker.
    pub async fn create_page_tracker_revision(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Option<TrackerDataRevision>> {
        let (resource, resource_group) = UtilsResource::WebScrapingPage.into();
        let Some(tracker) = self.get_page_tracker(tracker_id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %tracker_id,
                util.resource = resource,
                util.resource_group = resource_group,
                "Page tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "Page tracker ('{tracker_id}') is not found."
            )));
        };

        let RetrackTracker::Value(retrack) = tracker.retrack else {
            error!(
                user.id = %tracker.user_id,
                util.resource_id = %tracker.id,
                util.resource_name = tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %tracker.retrack.id(),
                "Page tracker is not found in Retrack."
            );
            bail!(SecutilsError::client(format!(
                "Page tracker ('{tracker_id}') is not found in Retrack."
            )));
        };

        // Enforce revisions limit and displace old ones.
        let features = self.user.subscription.get_features(&self.api.config);
        let max_revisions = std::cmp::min(
            retrack.config.revisions,
            features.config.web_scraping.tracker_revisions,
        );
        if max_revisions > 0 {
            self.api
                .retrack()
                .create_revision(retrack.id)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    }

    /// Returns all stored page tracker revisions.
    pub async fn get_page_tracker_history(
        &self,
        tracker_id: Uuid,
        params: PageTrackerGetHistoryParams,
    ) -> anyhow::Result<Vec<TrackerDataRevision>> {
        if params.refresh {
            self.create_page_tracker_revision(tracker_id).await?;
        }

        let Some(tracker) = self.get_page_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "Page tracker ('{tracker_id}') is not found."
            )));
        };

        self.api
            .retrack()
            .list_tracker_revisions(tracker.retrack.id(), Default::default())
            .await
    }

    /// Removes all persisted revisions for the specified page tracker.
    pub async fn clear_page_tracker_history(&self, tracker_id: Uuid) -> anyhow::Result<()> {
        let Some(tracker) = self.get_page_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "Page tracker ('{tracker_id}') is not found."
            )));
        };

        self.api
            .retrack()
            .clear_tracker_revisions(tracker.retrack.id())
            .await
    }

    fn validate_page_tracker_name(&self, name: &str) -> anyhow::Result<()> {
        if name.is_empty() {
            bail!(SecutilsError::client("Page tracker name cannot be empty."));
        }

        if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Page tracker name cannot be longer than {MAX_UTILS_ENTITY_NAME_LENGTH} characters.",
            )));
        }

        Ok(())
    }

    fn validate_page_tracker_config(&self, config: &PageTrackerConfig) -> anyhow::Result<()> {
        let features = self.user.subscription.get_features(&self.api.config);
        if config.revisions > features.config.web_scraping.tracker_revisions {
            bail!(SecutilsError::client(format!(
                "Page tracker revisions count cannot be greater than {}.",
                features.config.web_scraping.tracker_revisions
            )));
        }

        let Some(ref job_config) = config.job else {
            return Ok(());
        };

        // Validate that the schedule is a valid cron expression.
        let schedule = match Cron::parse_pattern(job_config.schedule.as_str()) {
            Ok(schedule) => schedule,
            Err(err) => {
                bail!(SecutilsError::client_with_root_cause(
                    anyhow!(
                        "Failed to parse schedule `{}`: {err:?}",
                        job_config.schedule
                    )
                    .context("Page tracker schedule must be a valid cron expression.")
                ));
            }
        };

        // Check if the interval between next occurrences is greater or equal to a minimum
        // interval defined by the subscription.
        let features = self.user.subscription.get_features(&self.api.config);
        let min_schedule_interval = schedule.min_interval()?;
        if min_schedule_interval < features.config.web_scraping.min_schedule_interval {
            bail!(SecutilsError::client(format!(
                "Page tracker schedule must have at least {} between occurrences, but detected {}.",
                humantime::format_duration(features.config.web_scraping.min_schedule_interval),
                humantime::format_duration(min_schedule_interval)
            )));
        }

        // Validate retry strategy.
        if let Some(retry_strategy) = &job_config.retry_strategy {
            let max_attempts = retry_strategy.max_attempts();
            if max_attempts == 0 || max_attempts > MAX_PAGE_TRACKER_RETRY_ATTEMPTS {
                bail!(SecutilsError::client(format!(
                    "Page tracker max retry attempts cannot be zero or greater than {MAX_PAGE_TRACKER_RETRY_ATTEMPTS}, but received {max_attempts}."
                )));
            }

            let min_interval = *retry_strategy.min_interval();
            if min_interval < MIN_PAGE_TRACKER_RETRY_INTERVAL {
                bail!(SecutilsError::client(format!(
                    "Page tracker min retry interval cannot be less than {}, but received {}.",
                    humantime::format_duration(MIN_PAGE_TRACKER_RETRY_INTERVAL),
                    humantime::format_duration(min_interval)
                )));
            }

            if let retrack_types::scheduler::SchedulerJobRetryStrategy::Linear {
                max_interval,
                ..
            }
            | retrack_types::scheduler::SchedulerJobRetryStrategy::Exponential {
                max_interval,
                ..
            } = retry_strategy
            {
                let max_interval = *max_interval;
                if max_interval < MIN_PAGE_TRACKER_RETRY_INTERVAL {
                    bail!(SecutilsError::client(format!(
                        "Page tracker retry strategy max interval cannot be less than {}, but received {}.",
                        humantime::format_duration(MIN_PAGE_TRACKER_RETRY_INTERVAL),
                        humantime::format_duration(max_interval)
                    )));
                }

                if max_interval > MAX_PAGE_TRACKER_RETRY_INTERVAL
                    || max_interval > min_schedule_interval
                {
                    bail!(SecutilsError::client(format!(
                        "Page tracker retry strategy max interval cannot be greater than {}, but received {}.",
                        humantime::format_duration(
                            MAX_PAGE_TRACKER_RETRY_INTERVAL.min(min_schedule_interval)
                        ),
                        humantime::format_duration(max_interval)
                    )));
                }
            }
        }

        Ok(())
    }

    fn validate_page_tracker_target(&self, target: &PageTrackerTarget) -> anyhow::Result<()> {
        if target.extractor.is_empty() {
            bail!(SecutilsError::client(
                "Page tracker extractor script cannot be empty."
            ));
        }

        Ok(())
    }

    /// Returns all API trackers.
    pub async fn get_api_trackers(&self) -> anyhow::Result<Vec<ApiTracker>> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let retrack = self.api.retrack();
        let utils_resource = UtilsResource::WebScrapingApi;
        let tags = [
            format!("{RETRACK_USER_TAG}:{}", self.user.id),
            format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
        ];
        let (mut trackers, retrack_trackers) = tokio::try_join!(
            web_scraping.get_api_trackers(),
            retrack.list_trackers(&tags)
        )?;

        let (resource, resource_group) = utils_resource.into();
        let mut retrack_trackers_map = retrack_trackers
            .into_iter()
            .map(|tracker| (tracker.id, tracker))
            .collect::<std::collections::HashMap<_, _>>();
        for tracker in trackers.iter_mut() {
            if let Some(retrack_tracker) = retrack_trackers_map.remove(&tracker.retrack.id()) {
                tracker.retrack = RetrackTracker::from_value(retrack_tracker);
            } else {
                error!(
                    user.id = %self.user.id,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    retrack.id = %tracker.retrack.id(),
                    "API tracker is not found in Retrack."
                );
            }
        }

        for retrack_tracker in retrack_trackers_map.values() {
            error!(
                user.id = %self.user.id,
                util.resource_id = get_tag_value(&retrack_tracker.tags, RETRACK_RESOURCE_ID_TAG),
                util.resource_name = retrack_tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %retrack_tracker.id,
                "Found a dangling Retrack tracker that needs to be removed."
            );
        }

        Ok(trackers)
    }

    /// Returns an API tracker by its ID.
    pub async fn get_api_tracker(&self, id: Uuid) -> anyhow::Result<Option<ApiTracker>> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let tracker = if let Some(mut tracker) = web_scraping.get_api_tracker(id).await? {
            if let Some(retrack_tracker) =
                self.api.retrack().get_tracker(tracker.retrack.id()).await?
            {
                tracker.retrack = RetrackTracker::from_value(retrack_tracker);
            } else {
                let (resource, resource_group) = UtilsResource::WebScrapingApi.into();
                error!(
                    user.id = %self.user.id,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    util.resource = resource,
                    util.resource_group = resource_group,
                    retrack.id = %tracker.retrack.id(),
                    "API tracker is not found in Retrack."
                );
            }

            Some(tracker)
        } else {
            None
        };

        Ok(tracker)
    }

    fn build_api_target_from_params(
        target: &ApiTrackerTarget,
        params: Option<serde_json::Value>,
    ) -> ApiTarget {
        ApiTarget {
            requests: vec![TargetRequest {
                url: target.url.clone(),
                method: target
                    .method
                    .as_deref()
                    .and_then(|m| m.parse::<Method>().ok()),
                headers: target.headers.as_ref().map(|h| {
                    let header_map: http::HeaderMap = h
                        .iter()
                        .filter_map(|(k, v)| {
                            let name = k.parse::<http::header::HeaderName>().ok()?;
                            let value = http::header::HeaderValue::from_str(v).ok()?;
                            Some((name, value))
                        })
                        .collect();
                    header_map
                }),
                body: target.body.clone(),
                media_type: target.media_type.as_ref().and_then(|m| m.parse().ok()),
                accept_statuses: target.accept_statuses.as_ref().map(|statuses| {
                    statuses
                        .iter()
                        .filter_map(|s| http::StatusCode::from_u16(*s).ok())
                        .collect::<HashSet<_>>()
                }),
                accept_invalid_certificates: target.accept_invalid_certificates,
            }],
            configurator: target.configurator.clone(),
            extractor: target.extractor.clone(),
            params,
        }
    }

    /// Creates a new API tracker.
    pub async fn create_api_tracker(
        &self,
        params: ApiTrackerCreateParams,
    ) -> anyhow::Result<ApiTracker> {
        self.validate_api_tracker_name(&params.name)?;
        self.validate_api_tracker_config(&params.config)?;
        self.validate_api_tracker_target(&params.target)?;

        let id = Uuid::now_v7();
        let retrack = self.api.retrack();
        let utils_resource = UtilsResource::WebScrapingApi;

        let api_params = if !params.secrets.is_none() {
            let secrets = self
                .api
                .secrets(self.user)
                .get_decrypted_secrets(&params.secrets)
                .await
                .unwrap_or_default();
            if secrets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "secrets": secrets }))
            }
        } else {
            None
        };

        let api_target = Self::build_api_target_from_params(&params.target, api_params);

        let retrack_tracker = retrack
            .create_tracker(&TrackerCreateParams {
                enabled: params.enabled,
                name: params.name.clone(),
                target: TrackerTarget::Api(api_target),
                config: TrackerConfig {
                    revisions: params.config.revisions,
                    timeout: None,
                    job: params.config.job,
                },
                tags: prepare_tags(&[
                    format!("{RETRACK_USER_TAG}:{}", self.user.id),
                    format!("{RETRACK_NOTIFICATIONS_TAG}:{}", params.notifications),
                    format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
                    format!("{RETRACK_RESOURCE_ID_TAG}:{id}"),
                    format!("{RETRACK_RESOURCE_NAME_TAG}:{}", params.name),
                ]),
                actions: vec![],
            })
            .await?;

        let created_at =
            OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())?;

        let tracker = ApiTracker {
            id,
            name: params.name,
            user_id: self.user.id,
            retrack: RetrackTracker::from_value(retrack_tracker),
            secrets: params.secrets.clone(),
            created_at,
            updated_at: created_at,
        };

        let web_scraping = self.api.db.web_scraping(self.user.id);
        if let Err(err) = web_scraping.insert_api_tracker(&tracker).await {
            if let Err(err) = retrack.remove_tracker(tracker.retrack.id()).await {
                let (resource, resource_group) = utils_resource.into();
                error!(
                    util.resource = resource,
                    util.resource_group = resource_group,
                    util.resource_id = %tracker.id,
                    util.resource_name = tracker.name,
                    retrack.id = %tracker.retrack.id(),
                    "Failed to remove tracker from Retrack: {err:?}"
                );
            }

            return Err(err);
        }

        Ok(tracker)
    }

    /// Updates existing API tracker.
    pub async fn update_api_tracker(
        &self,
        id: Uuid,
        params: ApiTrackerUpdateParams,
    ) -> anyhow::Result<ApiTracker> {
        let utils_resource = UtilsResource::WebScrapingApi;
        let (resource, resource_group) = utils_resource.into();
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let Some(existing_tracker) = web_scraping.get_api_tracker(id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %id,
                util.resource = resource,
                util.resource_group = resource_group,
                "API tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "API tracker ('{id}') is not found."
            )));
        };

        if let Some(ref name) = params.name {
            self.validate_api_tracker_name(name)?;
        }
        if let Some(ref config) = params.config {
            self.validate_api_tracker_config(config)?;
        }
        if let Some(ref target) = params.target {
            self.validate_api_tracker_target(target)?;
        }

        let retrack = self.api.retrack();
        let Some(retrack_tracker) = retrack.get_tracker(existing_tracker.retrack.id()).await?
        else {
            error!(
                user.id = %existing_tracker.user_id,
                util.resource_id = %existing_tracker.id,
                util.resource_name = existing_tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %existing_tracker.retrack.id(),
                "API tracker is not found in Retrack."
            );
            bail!(SecutilsError::client(format!(
                "API tracker ('{id}') is not found in Retrack."
            )));
        };

        let effective_secrets = params.secrets.as_ref().unwrap_or(&existing_tracker.secrets);
        let api_params = if !effective_secrets.is_none() {
            let secrets = self
                .api
                .secrets(self.user)
                .get_decrypted_secrets(effective_secrets)
                .await
                .unwrap_or_default();
            if secrets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "secrets": secrets }))
            }
        } else {
            None
        };

        let target_update = if let Some(target) = params.target {
            Some(TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest {
                    url: target.url.clone(),
                    method: target
                        .method
                        .as_deref()
                        .and_then(|m| m.parse::<Method>().ok()),
                    headers: target.headers.as_ref().map(|h| {
                        let header_map: http::HeaderMap = h
                            .iter()
                            .filter_map(|(k, v)| {
                                let name = k.parse::<http::header::HeaderName>().ok()?;
                                let value = http::header::HeaderValue::from_str(v).ok()?;
                                Some((name, value))
                            })
                            .collect();
                        header_map
                    }),
                    body: target.body.clone(),
                    media_type: target.media_type.as_ref().and_then(|m| m.parse().ok()),
                    accept_statuses: target.accept_statuses.as_ref().map(|statuses| {
                        statuses
                            .iter()
                            .filter_map(|s| http::StatusCode::from_u16(*s).ok())
                            .collect::<HashSet<_>>()
                    }),
                    accept_invalid_certificates: target.accept_invalid_certificates,
                }],
                configurator: target.configurator.clone(),
                extractor: target.extractor.clone(),
                params: api_params,
            }))
        } else if params.secrets.is_some() {
            Some(match &retrack_tracker.target {
                TrackerTarget::Api(api) => TrackerTarget::Api(ApiTarget {
                    params: api_params,
                    ..api.clone()
                }),
                other => other.clone(),
            })
        } else {
            None
        };

        let retrack_tracker = retrack
            .update_tracker(
                retrack_tracker.id,
                &TrackerUpdateParams {
                    name: params.name.clone(),
                    enabled: params.enabled,
                    config: params.config.map(|config| TrackerConfig {
                        revisions: config.revisions,
                        timeout: None,
                        job: config.job,
                    }),
                    target: target_update,
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", self.user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", params.notifications),
                        format!("{RETRACK_RESOURCE_TAG}:{utils_resource}"),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{id}"),
                        format!(
                            "{RETRACK_RESOURCE_NAME_TAG}:{}",
                            params.name.as_ref().unwrap_or(&existing_tracker.name)
                        ),
                    ])),
                    ..Default::default()
                },
            )
            .await?;

        let tracker = ApiTracker {
            name: params.name.unwrap_or(existing_tracker.name),
            retrack: RetrackTracker::from_value(retrack_tracker),
            secrets: params
                .secrets
                .clone()
                .unwrap_or(existing_tracker.secrets.clone()),
            updated_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            ..existing_tracker
        };

        web_scraping.update_api_tracker(&tracker).await?;

        Ok(tracker)
    }

    /// Removes existing API tracker and all history.
    pub async fn remove_api_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let (resource, resource_group) = UtilsResource::WebScrapingApi.into();

        let Some(tracker) = web_scraping.get_api_tracker(id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %id,
                util.resource = resource,
                util.resource_group = resource_group,
                "API tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "API tracker ('{id}') is not found."
            )));
        };

        let retrack = self.api.retrack();
        if let Some(retrack_tracker) = retrack.get_tracker(tracker.retrack.id()).await? {
            retrack.remove_tracker(retrack_tracker.id).await?;
        } else {
            error!(
                user.id = %tracker.user_id,
                util.resource_id = %tracker.id,
                util.resource_name = tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %tracker.retrack.id(),
                "API tracker is not found in Retrack, removing will be skipped."
            );
        };

        web_scraping.remove_api_tracker(id).await
    }

    /// Persists history for the specified API tracker.
    pub async fn create_api_tracker_revision(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Option<TrackerDataRevision>> {
        let (resource, resource_group) = UtilsResource::WebScrapingApi.into();
        let Some(tracker) = self.get_api_tracker(tracker_id).await? else {
            error!(
                user.id = %self.user.id,
                util.resource_id = %tracker_id,
                util.resource = resource,
                util.resource_group = resource_group,
                "API tracker is not found."
            );
            bail!(SecutilsError::client(format!(
                "API tracker ('{tracker_id}') is not found."
            )));
        };

        let RetrackTracker::Value(retrack) = tracker.retrack else {
            error!(
                user.id = %tracker.user_id,
                util.resource_id = %tracker.id,
                util.resource_name = tracker.name,
                util.resource = resource,
                util.resource_group = resource_group,
                retrack.id = %tracker.retrack.id(),
                "API tracker is not found in Retrack."
            );
            bail!(SecutilsError::client(format!(
                "API tracker ('{tracker_id}') is not found in Retrack."
            )));
        };

        let features = self.user.subscription.get_features(&self.api.config);
        let max_revisions = std::cmp::min(
            retrack.config.revisions,
            features.config.web_scraping.tracker_revisions,
        );
        if max_revisions > 0 {
            self.api
                .retrack()
                .create_revision(retrack.id)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    }

    /// Returns all stored API tracker revisions.
    pub async fn get_api_tracker_history(
        &self,
        tracker_id: Uuid,
        params: PageTrackerGetHistoryParams,
    ) -> anyhow::Result<Vec<TrackerDataRevision>> {
        if params.refresh {
            self.create_api_tracker_revision(tracker_id).await?;
        }

        let Some(tracker) = self.get_api_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "API tracker ('{tracker_id}') is not found."
            )));
        };

        self.api
            .retrack()
            .list_tracker_revisions(tracker.retrack.id(), Default::default())
            .await
    }

    /// Removes all persisted revisions for the specified API tracker.
    pub async fn clear_api_tracker_history(&self, tracker_id: Uuid) -> anyhow::Result<()> {
        let Some(tracker) = self.get_api_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "API tracker ('{tracker_id}') is not found."
            )));
        };

        self.api
            .retrack()
            .clear_tracker_revisions(tracker.retrack.id())
            .await
    }

    /// Sends a test HTTP request using the provided API tracker target configuration and returns
    /// the response status, headers, body, and latency.
    pub async fn test_api_request(
        &self,
        params: ApiTrackerTestParams,
    ) -> anyhow::Result<ApiTrackerTestResult> {
        self.validate_api_tracker_target(&params.target)?;

        let target = &params.target;
        let method: reqwest::Method = target
            .method
            .as_deref()
            .unwrap_or("GET")
            .parse()
            .map_err(|_| SecutilsError::client("Invalid HTTP method."))?;

        let mut request = self
            .api
            .network
            .http_client
            .request(method, target.url.as_str());

        if let Some(ref headers) = target.headers {
            for (key, value) in headers {
                request = request.header(key.as_str(), value.as_str());
            }
        }

        if let Some(ref body) = target.body {
            request = request.body(body.to_string());
            if target.media_type.is_none()
                && target
                    .headers
                    .as_ref()
                    .is_none_or(|h| !h.keys().any(|k| k.eq_ignore_ascii_case("content-type")))
            {
                request = request.header("content-type", "application/json");
            }
        }

        let start = std::time::Instant::now();
        let response = request
            .send()
            .await
            .map_err(|err| SecutilsError::client(format!("Request failed: {err}")))?;
        let latency_ms = start.elapsed().as_millis() as u64;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
            .collect();

        const MAX_BODY_SIZE: usize = 512 * 1024;
        let body_bytes = response
            .bytes()
            .await
            .map_err(|err| SecutilsError::client(format!("Failed to read response body: {err}")))?;
        let body = if body_bytes.len() > MAX_BODY_SIZE {
            String::from_utf8_lossy(&body_bytes[..MAX_BODY_SIZE]).into_owned()
        } else {
            String::from_utf8_lossy(&body_bytes).into_owned()
        };

        Ok(ApiTrackerTestResult {
            status,
            headers,
            body,
            latency_ms,
        })
    }

    fn validate_api_tracker_name(&self, name: &str) -> anyhow::Result<()> {
        if name.is_empty() {
            bail!(SecutilsError::client("API tracker name cannot be empty."));
        }

        if name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "API tracker name cannot be longer than {MAX_UTILS_ENTITY_NAME_LENGTH} characters.",
            )));
        }

        Ok(())
    }

    fn validate_api_tracker_config(&self, config: &ApiTrackerConfig) -> anyhow::Result<()> {
        let features = self.user.subscription.get_features(&self.api.config);
        if config.revisions > features.config.web_scraping.tracker_revisions {
            bail!(SecutilsError::client(format!(
                "API tracker revisions count cannot be greater than {}.",
                features.config.web_scraping.tracker_revisions
            )));
        }

        let Some(ref job_config) = config.job else {
            return Ok(());
        };

        let schedule = match Cron::parse_pattern(job_config.schedule.as_str()) {
            Ok(schedule) => schedule,
            Err(err) => {
                bail!(SecutilsError::client_with_root_cause(
                    anyhow!(
                        "Failed to parse schedule `{}`: {err:?}",
                        job_config.schedule
                    )
                    .context("API tracker schedule must be a valid cron expression.")
                ));
            }
        };

        let features = self.user.subscription.get_features(&self.api.config);
        let min_schedule_interval = schedule.min_interval()?;
        if min_schedule_interval < features.config.web_scraping.min_schedule_interval {
            bail!(SecutilsError::client(format!(
                "API tracker schedule must have at least {} between occurrences, but detected {}.",
                humantime::format_duration(features.config.web_scraping.min_schedule_interval),
                humantime::format_duration(min_schedule_interval)
            )));
        }

        if let Some(retry_strategy) = &job_config.retry_strategy {
            let max_attempts = retry_strategy.max_attempts();
            if max_attempts == 0 || max_attempts > MAX_API_TRACKER_RETRY_ATTEMPTS {
                bail!(SecutilsError::client(format!(
                    "API tracker max retry attempts cannot be zero or greater than {MAX_API_TRACKER_RETRY_ATTEMPTS}, but received {max_attempts}."
                )));
            }

            let min_interval = *retry_strategy.min_interval();
            if min_interval < MIN_API_TRACKER_RETRY_INTERVAL {
                bail!(SecutilsError::client(format!(
                    "API tracker min retry interval cannot be less than {}, but received {}.",
                    humantime::format_duration(MIN_API_TRACKER_RETRY_INTERVAL),
                    humantime::format_duration(min_interval)
                )));
            }

            if let retrack_types::scheduler::SchedulerJobRetryStrategy::Linear {
                max_interval,
                ..
            }
            | retrack_types::scheduler::SchedulerJobRetryStrategy::Exponential {
                max_interval,
                ..
            } = retry_strategy
            {
                let max_interval = *max_interval;
                if max_interval < MIN_API_TRACKER_RETRY_INTERVAL {
                    bail!(SecutilsError::client(format!(
                        "API tracker retry strategy max interval cannot be less than {}, but received {}.",
                        humantime::format_duration(MIN_API_TRACKER_RETRY_INTERVAL),
                        humantime::format_duration(max_interval)
                    )));
                }

                if max_interval > MAX_API_TRACKER_RETRY_INTERVAL
                    || max_interval > min_schedule_interval
                {
                    bail!(SecutilsError::client(format!(
                        "API tracker retry strategy max interval cannot be greater than {}, but received {}.",
                        humantime::format_duration(
                            MAX_API_TRACKER_RETRY_INTERVAL.min(min_schedule_interval)
                        ),
                        humantime::format_duration(max_interval)
                    )));
                }
            }
        }

        Ok(())
    }

    fn validate_api_tracker_target(&self, target: &ApiTrackerTarget) -> anyhow::Result<()> {
        let scheme = target.url.scheme();
        if scheme != "http" && scheme != "https" {
            bail!(SecutilsError::client(
                "API tracker URL must use http or https scheme."
            ));
        }

        if let Some(ref configurator) = target.configurator
            && configurator.is_empty()
        {
            bail!(SecutilsError::client(
                "API tracker configurator script cannot be empty when provided."
            ));
        }

        if let Some(ref extractor) = target.extractor
            && extractor.is_empty()
        {
            bail!(SecutilsError::client(
                "API tracker extractor script cannot be empty when provided."
            ));
        }

        Ok(())
    }

    /// Syncs secrets to all page trackers that use secrets (SecretsAccess != None).
    /// Called when a user creates, updates, or deletes a secret.
    pub async fn sync_secrets_to_trackers(&self) -> anyhow::Result<()> {
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let trackers = web_scraping.get_page_trackers().await?;
        let trackers_with_secrets: Vec<_> = trackers
            .into_iter()
            .filter(|t| !t.secrets.is_none())
            .collect();
        if trackers_with_secrets.is_empty() {
            return Ok(());
        }

        let retrack = self.api.retrack();
        for tracker in trackers_with_secrets {
            let secrets = self
                .api
                .secrets(self.user)
                .get_decrypted_secrets(&tracker.secrets)
                .await
                .unwrap_or_default();
            let params_json = if secrets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "secrets": secrets }))
            };

            let Some(retrack_tracker) = retrack.get_tracker(tracker.retrack.id()).await? else {
                continue;
            };

            let update_params = TrackerUpdateParams {
                target: Some(match retrack_tracker.target {
                    TrackerTarget::Page(page) => TrackerTarget::Page(PageTarget {
                        params: params_json,
                        ..page
                    }),
                    other => other,
                }),
                ..Default::default()
            };
            if let Err(err) = retrack
                .update_tracker(retrack_tracker.id, &update_params)
                .await
            {
                error!(
                    user.id = %self.user.id,
                    retrack.id = %tracker.retrack.id(),
                    "Failed to sync secrets to tracker: {err:?}"
                );
            }
        }

        // Sync API trackers
        let api_trackers = web_scraping.get_api_trackers().await?;
        let api_trackers_with_secrets: Vec<_> = api_trackers
            .into_iter()
            .filter(|t| !t.secrets.is_none())
            .collect();
        for tracker in api_trackers_with_secrets {
            let secrets = self
                .api
                .secrets(self.user)
                .get_decrypted_secrets(&tracker.secrets)
                .await
                .unwrap_or_default();
            let params_json = if secrets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "secrets": secrets }))
            };

            let Some(retrack_tracker) = retrack.get_tracker(tracker.retrack.id()).await? else {
                continue;
            };

            let update_params = TrackerUpdateParams {
                target: Some(match &retrack_tracker.target {
                    TrackerTarget::Api(api) => TrackerTarget::Api(ApiTarget {
                        params: params_json,
                        ..api.clone()
                    }),
                    other => other.clone(),
                }),
                ..Default::default()
            };
            if let Err(err) = retrack
                .update_tracker(retrack_tracker.id, &update_params)
                .await
            {
                error!(
                    user.id = %self.user.id,
                    retrack.id = %tracker.retrack.id(),
                    "Failed to sync secrets to API tracker: {err:?}"
                );
            }
        }

        Ok(())
    }
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data.
    pub fn web_scraping(&'a self, user: &'u User) -> WebScrapingApiExt<'a, 'u, DR, ET> {
        WebScrapingApiExt::new(self, user)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApiTrackerCreateParams, ApiTrackerTestParams, ApiTrackerUpdateParams,
        PageTrackerGetHistoryParams, PageTrackerUpdateParams, RETRACK_NOTIFICATIONS_TAG,
        RETRACK_RESOURCE_ID_TAG, RETRACK_RESOURCE_NAME_TAG, RETRACK_RESOURCE_TAG, RETRACK_USER_TAG,
    };
    use crate::{
        error::Error as SecutilsError,
        retrack::{
            RetrackTracker,
            tags::prepare_tags,
            tests::{RetrackTrackerValue, mock_retrack_tracker},
        },
        tests::{mock_api, mock_api_with_config, mock_config, mock_user},
        utils::{
            UtilsResource,
            web_scraping::{
                ApiTracker, ApiTrackerConfig, ApiTrackerTarget, PageTracker, PageTrackerConfig,
                PageTrackerTarget, api_ext::PageTrackerCreateParams,
            },
        },
    };
    use actix_web::ResponseError;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use retrack_types::{
        scheduler::{SchedulerJobConfig, SchedulerJobRetryStrategy},
        trackers::{
            ApiTarget, PageTarget, TargetRequest, Tracker, TrackerConfig, TrackerCreateParams,
            TrackerDataRevision, TrackerDataValue, TrackerTarget, TrackerUpdateParams,
        },
    };
    use serde_json::json;
    use sqlx::PgPool;
    use std::time::Duration;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[sqlx::test]
    async fn properly_creates_new_page_tracker(pool: PgPool) -> anyhow::Result<()> {
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
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage)
                        ]),
                        actions: vec![],
                    })
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
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
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        assert_eq!(
            tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert();

        assert_eq!(
            tracker.retrack,
            RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: retrack_tracker.id,
                enabled: retrack_tracker.enabled,
                config: retrack_tracker.config.clone(),
                target: retrack_tracker.target.clone(),
                notifications: false,
            }))
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_page_tracker_at_creation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool.clone()).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);

        let job_config = SchedulerJobConfig {
            schedule: "@hourly".to_string(),
            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(120),
                max_attempts: 5,
            }),
        };
        let config = PageTrackerConfig {
            revisions: 3,
            job: Some(job_config.clone()),
        };
        let target = PageTrackerTarget {
            extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
        };

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "".to_string(),
                enabled: true,
                config: config.clone(),
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "a".repeat(101),
                enabled: true,
                config: config.clone(),
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 31,
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker revisions count cannot be greater than 30.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "-".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###"
        Error {
            context: "Page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Invalid pattern: Pattern must have 6 or 7 fields when seconds are required and years are optional.",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "0/5 * * * * *".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker schedule must have at least 10s between occurrences, but detected 5s.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 0,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 11,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@monthly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(13 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(2 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Empty extractor.
        assert_debug_snapshot!(
            create_and_fail(api.create_page_tracker(PageTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: config.clone(),
                target: PageTrackerTarget {
                    extractor: "".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""Page tracker extractor script cannot be empty.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_updates_page_tracker(pool: PgPool) -> anyhow::Result<()> {
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
                            job: None,
                        },
                        tags: prepare_tags(&[
                            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage)
                        ]),
                        actions: vec![],
                    })
                        .unwrap(),
                );
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
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        // Update name.
        let updated_retrack_tracker = Tracker {
            name: "name_two".to_string(),
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            // Use partial body match due to a non-deterministic tag with new tracker ID.
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_page_tracker(
                tracker.id,
                PageTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = PageTracker {
            name: "name_two".to_string(),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);

        assert_eq!(
            expected_tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update config.
        let updated_retrack_tracker = Tracker {
            config: TrackerConfig {
                revisions: 4,
                timeout: None,
                job: None,
            },
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            // Use partial body match due to a non-deterministic tag with new tracker ID.
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    config: Some(updated_retrack_tracker.config.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_page_tracker(
                tracker.id,
                PageTrackerUpdateParams {
                    config: Some(PageTrackerConfig {
                        revisions: 4,
                        job: None,
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = PageTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update job config.
        let updated_retrack_tracker = Tracker {
            config: TrackerConfig {
                revisions: 4,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                }),
            },
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            // Use partial body match due to a non-deterministic tag with new tracker ID.
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    config: Some(updated_retrack_tracker.config.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_page_tracker(
                tracker.id,
                PageTrackerUpdateParams {
                    config: Some(PageTrackerConfig {
                        revisions: 4,
                        job: Some(SchedulerJobConfig {
                            schedule: "@hourly".to_string(),
                            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                                interval: Duration::from_secs(120),
                                max_attempts: 5,
                            }),
                        }),
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = PageTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update target.
        let updated_retrack_tracker = Tracker {
            target: TrackerTarget::Page(PageTarget {
                extractor: "export async function execute(p) { await p.goto('http://localhost:1234/my/app?q=3'); return await p.content(); }".to_string(),
                params: None,
                engine: None,
                user_agent: None,
                accept_invalid_certificates: false,
            }),
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    target: Some(updated_retrack_tracker.target.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_page_tracker(
                tracker.id,
                PageTrackerUpdateParams {
                    target: Some(PageTrackerTarget {
                        extractor: "export async function execute(p) { await p.goto('http://localhost:1234/my/app?q=3'); return await p.content(); }".to_string(),
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = PageTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update notifications settings.
        let updated_retrack_tracker = Tracker {
            tags: prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_NOTIFICATIONS_TAG}:{}", true),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                format!("{RETRACK_RESOURCE_NAME_TAG}:{}", expected_tracker.name),
            ]),
            ..retrack_tracker.clone()
        };
        let retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", true),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
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
        let updated_tracker = web_scraping
            .update_page_tracker(
                tracker.id,
                PageTrackerUpdateParams {
                    notifications: true,
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();

        let expected_tracker = PageTracker {
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: true,
            })),
            updated_at: updated_tracker.updated_at,
            ..expected_tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_page_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_page_tracker_at_update(pool: PgPool) -> anyhow::Result<()> {
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
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                            format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage)
                        ]),
                        actions: vec![],
                    })
                        .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let job_config = SchedulerJobConfig {
            schedule: "@hourly".to_string(),
            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(120),
                max_attempts: 5,
            }),
        };
        let config = PageTrackerConfig {
            revisions: 3,
            job: Some(job_config.clone()),
        };

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Non-existent tracker.
        let update_result = update_and_fail(
            web_scraping
                .update_page_tracker(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    PageTrackerUpdateParams {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Page tracker ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                name: Some("".to_string()),
                ..Default::default()
            }).await),
            @r###""Page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                name: Some("a".repeat(101)),
                ..Default::default()
            }).await),
            @r###""Page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    revisions: 31,
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker revisions count cannot be greater than 30.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "-".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###"
        Error {
            context: "Page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Invalid pattern: Pattern must have 6 or 7 fields when seconds are required and years are optional.",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "0/5 * * * * *".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker schedule must have at least 10s between occurrences, but detected 5s.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 0,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                 config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 11,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
           update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@monthly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(13 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                 config: Some(PageTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(2 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                }),
               ..Default::default()
            }).await),
            @r###""Page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Empty extractor.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_page_tracker(tracker.id, PageTrackerUpdateParams {
                target: Some(PageTrackerTarget {
                    extractor: "".to_string(),
                }),
                ..Default::default()
            }).await),
            @r###""Page tracker extractor script cannot be empty.""###
        );

        // Non-existent retrack tracker.
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(404);
        });
        let update_result = update_and_fail(
            web_scraping
                .update_page_tracker(tracker.id, Default::default())
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!("Page tracker ('{}') is not found in Retrack.", tracker.id)
        );
        retrack_get_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_removes_page_trackers(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let retrack_tracker_one = mock_retrack_tracker()?;
        let mut retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let tracker_one = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();
        retrack_create_api_mock.delete();

        let mut retrack_tracker_two = mock_retrack_tracker()?;
        retrack_tracker_two.id = uuid!("00000000-0000-0000-0000-000000000020");
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let tracker_two = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_two".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_one.clone(), retrack_tracker_two.clone()]);
        });
        assert_eq!(
            web_scraping.get_page_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker_one.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let mut retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker_one.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_two.clone()]);
        });
        web_scraping.remove_page_tracker(tracker_one.id).await?;
        assert_eq!(
            web_scraping.get_page_trackers().await?,
            vec![tracker_two.clone()],
        );
        retrack_get_api_mock.assert();
        retrack_get_api_mock.delete();
        retrack_delete_api_mock.assert();
        retrack_delete_api_mock.delete();
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker_two.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker_two.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Vec::<Tracker>::new());
        });
        web_scraping.remove_page_tracker(tracker_two.id).await?;
        assert!(web_scraping.get_page_trackers().await?.is_empty());
        retrack_get_api_mock.assert();
        retrack_delete_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_page_trackers_by_id(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(
            web_scraping
                .get_page_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
                .await?
                .is_none()
        );

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
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
        assert_eq!(
            web_scraping.get_page_tracker(tracker.id).await?,
            Some(tracker.clone()),
        );
        retrack_get_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_all_page_trackers(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let retrack_tracker_one = mock_retrack_tracker()?;
        let mut retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let tracker_one = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();
        retrack_create_api_mock.delete();

        let mut retrack_tracker_two = mock_retrack_tracker()?;
        retrack_tracker_two.id = uuid!("00000000-0000-0000-0000-000000000020");
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let tracker_two = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_two".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingPage),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_one.clone(), retrack_tracker_two.clone()]);
        });
        assert_eq!(
            web_scraping.get_page_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_saves_page_revision(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let mut retrack_list_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id))
                .query_param("calculateDiff", "false");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Vec::<TrackerDataRevision>::new());
        });
        let tracker_history = web_scraping
            .get_page_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_history.is_empty());
        retrack_list_revisions_api_mock.assert();
        retrack_list_revisions_api_mock.delete();

        let retrack_create_revision_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&TrackerDataRevision {
                    id: uuid!("00000000-0000-0000-0000-000000000100"),
                    tracker_id: retrack_tracker.id,
                    created_at: OffsetDateTime::from_unix_timestamp(946720800).unwrap(),
                    data: TrackerDataValue::new(json!({ "one": 1 })),
                });
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
        let tracker_history = web_scraping
            .get_page_tracker_history(tracker.id, PageTrackerGetHistoryParams { refresh: true })
            .await?;
        assert_eq!(
            tracker_history,
            vec![TrackerDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000100"),
                tracker_id: retrack_tracker.id,
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                data: TrackerDataValue::new(json!({ "one": 1 })),
            }]
        );
        retrack_get_api_mock.assert_calls(3);
        retrack_create_revision_api_mock.assert();
        retrack_list_revisions_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_forwards_error_if_page_content_extraction_fails(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_create_revision_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "some client-error".to_string() }));
        });
        let scraper_error = web_scraping
            .get_page_tracker_history(tracker.id, PageTrackerGetHistoryParams { refresh: true })
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()?;

        retrack_get_api_mock.assert();
        retrack_create_revision_api_mock.assert();

        assert_eq!(scraper_error.status_code(), 400);
        assert_debug_snapshot!(
            scraper_error,
            @r###""{\"message\":\"some client-error\"}""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_clears_page_tracker_revision_history(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        // Insert a new user to the database.
        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let retrack_tracker = mock_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_page_tracker(PageTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: PageTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: PageTrackerTarget {
                    extractor: "export async function execute(p) { await p.goto('https://secutils.dev/'); return await p.content(); }".to_string(),
                },
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_clear_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(204).header("Content-Type", "application/json");
        });

        web_scraping.clear_page_tracker_history(tracker.id).await?;

        retrack_create_api_mock.assert();
        retrack_get_api_mock.assert();
        retrack_clear_revisions_api_mock.assert();

        Ok(())
    }

    fn mock_api_retrack_tracker() -> anyhow::Result<Tracker> {
        Ok(Tracker {
            id: uuid!("00000000-0000-0000-0000-000000000010"),
            name: "name_one".to_string(),
            enabled: true,
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                }],
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

    #[sqlx::test]
    async fn properly_creates_new_api_tracker(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_api_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers")
                .json_body_includes(
                    serde_json::to_string_pretty(&TrackerCreateParams {
                        name: "name_one".to_string(),
                        enabled: true,
                        target: TrackerTarget::Api(ApiTarget {
                            requests: vec![TargetRequest {
                                url: "https://api.example.com/data".parse().unwrap(),
                                method: None,
                                headers: None,
                                body: None,
                                media_type: None,
                                accept_statuses: None,
                                accept_invalid_certificates: false,
                            }],
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
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
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
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        assert_eq!(
            tracker,
            web_scraping.get_api_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert();

        assert_eq!(
            tracker.retrack,
            RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: retrack_tracker.id,
                enabled: retrack_tracker.enabled,
                config: retrack_tracker.config.clone(),
                target: retrack_tracker.target.clone(),
                notifications: false,
            }))
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_api_tracker_at_creation(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool.clone()).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);

        let job_config = SchedulerJobConfig {
            schedule: "@hourly".to_string(),
            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(120),
                max_attempts: 5,
            }),
        };
        let config = ApiTrackerConfig {
            revisions: 3,
            job: Some(job_config.clone()),
        };
        let target = ApiTrackerTarget {
            url: "https://api.example.com/data".parse()?,
            method: None,
            headers: None,
            body: None,
            media_type: None,
            accept_statuses: None,
            accept_invalid_certificates: false,
            configurator: None,
            extractor: None,
        };

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "".to_string(),
                enabled: true,
                config: config.clone(),
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "a".repeat(101),
                enabled: true,
                config: config.clone(),
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 31,
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker revisions count cannot be greater than 30.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "-".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###"
        Error {
            context: "API tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Invalid pattern: Pattern must have 6 or 7 fields when seconds are required and years are optional.",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "0/5 * * * * *".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker schedule must have at least 10s between occurrences, but detected 5s.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 0,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 11,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval (monthly schedule).
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@monthly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(13 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        // Too high max retry interval (hourly schedule).
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(2 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                },
                target: target.clone(),
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL scheme.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: config.clone(),
                target: ApiTrackerTarget {
                    url: "ftp://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker URL must use http or https scheme.""###
        );

        // Empty configurator.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: config.clone(),
                target: ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: Some("".to_string()),
                    extractor: None,
                },
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker configurator script cannot be empty when provided.""###
        );

        // Empty extractor.
        assert_debug_snapshot!(
            create_and_fail(api.create_api_tracker(ApiTrackerCreateParams {
                name: "name".to_string(),
                enabled: true,
                config: config.clone(),
                target: ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: Some("".to_string()),
                },
                notifications: false,
                secrets: Default::default(),
            }).await),
            @r###""API tracker extractor script cannot be empty when provided.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn properly_updates_api_tracker(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_api_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers")
                .json_body_includes(
                    serde_json::to_string_pretty(&TrackerCreateParams {
                        name: "name_one".to_string(),
                        enabled: true,
                        target: TrackerTarget::Api(ApiTarget {
                            requests: vec![TargetRequest {
                                url: "https://api.example.com/data".parse().unwrap(),
                                method: None,
                                headers: None,
                                body: None,
                                media_type: None,
                                accept_statuses: None,
                                accept_invalid_certificates: false,
                            }],
                            configurator: None,
                            extractor: None,
                            params: None,
                        }),
                        config: TrackerConfig {
                            revisions: 3,
                            timeout: None,
                            job: None,
                        },
                        tags: prepare_tags(&[
                            format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
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

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        // Update name.
        let updated_retrack_tracker = Tracker {
            name: "name_two".to_string(),
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_api_tracker(
                tracker.id,
                ApiTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = ApiTracker {
            name: "name_two".to_string(),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);

        assert_eq!(
            expected_tracker,
            web_scraping.get_api_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update config.
        let updated_retrack_tracker = Tracker {
            config: TrackerConfig {
                revisions: 4,
                timeout: None,
                job: None,
            },
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    config: Some(updated_retrack_tracker.config.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_api_tracker(
                tracker.id,
                ApiTrackerUpdateParams {
                    config: Some(ApiTrackerConfig {
                        revisions: 4,
                        job: None,
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = ApiTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_api_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update job config.
        let updated_retrack_tracker = Tracker {
            config: TrackerConfig {
                revisions: 4,
                timeout: None,
                job: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                }),
            },
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    config: Some(updated_retrack_tracker.config.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_api_tracker(
                tracker.id,
                ApiTrackerUpdateParams {
                    config: Some(ApiTrackerConfig {
                        revisions: 4,
                        job: Some(SchedulerJobConfig {
                            schedule: "@hourly".to_string(),
                            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                                interval: Duration::from_secs(120),
                                max_attempts: 5,
                            }),
                        }),
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = ApiTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_api_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        // Update target.
        let updated_retrack_tracker = Tracker {
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest {
                    url: "https://api.example.com/other".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                }],
                configurator: None,
                extractor: None,
                params: None,
            }),
            ..retrack_tracker.clone()
        };
        let mut retrack_update_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path(format!("/api/trackers/{}", retrack_tracker.id))
                .json_body_obj(&TrackerUpdateParams {
                    target: Some(updated_retrack_tracker.target.clone()),
                    tags: Some(prepare_tags(&[
                        format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                        format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
                        format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
                        format!("{RETRACK_RESOURCE_ID_TAG}:{}", tracker.id),
                        format!("{RETRACK_RESOURCE_NAME_TAG}:name_two"),
                    ])),
                    ..Default::default()
                });
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&updated_retrack_tracker);
        });
        let updated_tracker = web_scraping
            .update_api_tracker(
                tracker.id,
                ApiTrackerUpdateParams {
                    target: Some(ApiTrackerTarget {
                        url: "https://api.example.com/other".parse()?,
                        method: None,
                        headers: None,
                        body: None,
                        media_type: None,
                        accept_statuses: None,
                        accept_invalid_certificates: false,
                        configurator: None,
                        extractor: None,
                    }),
                    ..Default::default()
                },
            )
            .await?;
        retrack_update_api_mock.assert();
        retrack_update_api_mock.delete();

        let expected_tracker = ApiTracker {
            name: "name_two".to_string(),
            retrack: RetrackTracker::Value(Box::new(RetrackTrackerValue {
                id: updated_retrack_tracker.id,
                enabled: updated_retrack_tracker.enabled,
                config: updated_retrack_tracker.config.clone(),
                target: updated_retrack_tracker.target.clone(),
                notifications: false,
            })),
            updated_at: updated_tracker.updated_at,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_api_tracker(tracker.id).await?.unwrap()
        );
        retrack_get_api_mock.assert_calls(2);
        retrack_get_api_mock.delete();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_validates_api_tracker_at_update(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let retrack_tracker = mock_api_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/trackers")
                .json_body_includes(
                    serde_json::to_string_pretty(&TrackerCreateParams {
                        name: "name_one".to_string(),
                        enabled: true,
                        target: TrackerTarget::Api(ApiTarget {
                            requests: vec![TargetRequest {
                                url: "https://api.example.com/data".parse().unwrap(),
                                method: None,
                                headers: None,
                                body: None,
                                media_type: None,
                                accept_statuses: None,
                                accept_invalid_certificates: false,
                            }],
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
                            format!("{RETRACK_NOTIFICATIONS_TAG}:{}", false),
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

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let job_config = SchedulerJobConfig {
            schedule: "@hourly".to_string(),
            retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                interval: Duration::from_secs(120),
                max_attempts: 5,
            }),
        };
        let config = ApiTrackerConfig {
            revisions: 3,
            job: Some(job_config.clone()),
        };

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                    }),
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Non-existent tracker.
        let update_result = update_and_fail(
            web_scraping
                .update_api_tracker(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    ApiTrackerUpdateParams {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            "API tracker ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                name: Some("".to_string()),
                ..Default::default()
            }).await),
            @r###""API tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                name: Some("a".repeat(101)),
                ..Default::default()
            }).await),
            @r###""API tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    revisions: 31,
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker revisions count cannot be greater than 30.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "-".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###"
        Error {
            context: "API tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Invalid pattern: Pattern must have 6 or 7 fields when seconds are required and years are optional.",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "0/5 * * * * *".to_string(),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker schedule must have at least 10s between occurrences, but detected 5s.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 0,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 11,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(30),
                            max_attempts: 5,
                        }),
                        ..job_config.clone()
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@monthly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(13 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                config: Some(ApiTrackerConfig {
                    job: Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                            initial_interval: Duration::from_secs(120),
                            increment: Duration::from_secs(10),
                            max_interval: Duration::from_secs(2 * 3600),
                            max_attempts: 5,
                        }),
                    }),
                    ..config.clone()
                }),
                ..Default::default()
            }).await),
            @r###""API tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL scheme.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                target: Some(ApiTrackerTarget {
                    url: "ftp://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                }),
                ..Default::default()
            }).await),
            @r###""API tracker URL must use http or https scheme.""###
        );

        // Empty configurator.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                target: Some(ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: Some("".to_string()),
                    extractor: None,
                }),
                ..Default::default()
            }).await),
            @r###""API tracker configurator script cannot be empty when provided.""###
        );

        // Empty extractor.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_api_tracker(tracker.id, ApiTrackerUpdateParams {
                target: Some(ApiTrackerTarget {
                    url: "https://api.example.com/data".parse()?,
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: Some("".to_string()),
                }),
                ..Default::default()
            }).await),
            @r###""API tracker extractor script cannot be empty when provided.""###
        );

        // Non-existent retrack tracker.
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(404);
        });
        let update_result = update_and_fail(
            web_scraping
                .update_api_tracker(tracker.id, Default::default())
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!("API tracker ('{}') is not found in Retrack.", tracker.id)
        );
        retrack_get_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_removes_api_trackers(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let retrack_tracker_one = mock_api_retrack_tracker()?;
        let mut retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let tracker_one = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();
        retrack_create_api_mock.delete();

        let mut retrack_tracker_two = mock_api_retrack_tracker()?;
        retrack_tracker_two.id = uuid!("00000000-0000-0000-0000-000000000020");
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let tracker_two = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_two".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_one.clone(), retrack_tracker_two.clone()]);
        });
        assert_eq!(
            web_scraping.get_api_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let mut retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker_one.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let mut retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker_one.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let mut retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_two.clone()]);
        });
        web_scraping.remove_api_tracker(tracker_one.id).await?;
        assert_eq!(
            web_scraping.get_api_trackers().await?,
            vec![tracker_two.clone()],
        );
        retrack_get_api_mock.assert();
        retrack_get_api_mock.delete();
        retrack_delete_api_mock.assert();
        retrack_delete_api_mock.delete();
        retrack_list_api_mock.assert();
        retrack_list_api_mock.delete();

        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker_two.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let retrack_delete_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}", retrack_tracker_two.id));
            then.status(200).header("Content-Type", "application/json");
        });
        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&Vec::<Tracker>::new());
        });
        web_scraping.remove_api_tracker(tracker_two.id).await?;
        assert!(web_scraping.get_api_trackers().await?.is_empty());
        retrack_get_api_mock.assert();
        retrack_delete_api_mock.assert();
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_api_trackers_by_id(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(
            web_scraping
                .get_api_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
                .await?
                .is_none()
        );

        let retrack_tracker = mock_api_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
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
        assert_eq!(
            web_scraping.get_api_tracker(tracker.id).await?,
            Some(tracker.clone()),
        );
        retrack_get_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_returns_all_api_trackers(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let retrack_tracker_one = mock_api_retrack_tracker()?;
        let mut retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_one);
        });
        let tracker_one = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();
        retrack_create_api_mock.delete();

        let mut retrack_tracker_two = mock_api_retrack_tracker()?;
        retrack_tracker_two.id = uuid!("00000000-0000-0000-0000-000000000020");
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker_two);
        });
        let tracker_two = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_two".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_list_api_mock = retrack_server.mock(|when, then| {
            let tags = prepare_tags(&[
                format!("{RETRACK_USER_TAG}:{}", mock_user.id),
                format!("{RETRACK_RESOURCE_TAG}:{}", UtilsResource::WebScrapingApi),
            ])
            .into_iter()
            .collect::<Vec<_>>();
            when.method(httpmock::Method::GET)
                .path("/api/trackers")
                .query_param("tag", &tags[0])
                .query_param("tag", &tags[1])
                .query_param("tag", &tags[2]);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&[retrack_tracker_one.clone(), retrack_tracker_two.clone()]);
        });
        assert_eq!(
            web_scraping.get_api_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );
        retrack_list_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_clears_api_tracker_revision_history(pool: PgPool) -> anyhow::Result<()> {
        let mut config = mock_config()?;
        let mock_user = mock_user()?;

        let retrack_server = MockServer::start();
        config.retrack.host = Url::parse(&retrack_server.base_url())?;

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let retrack_tracker = mock_api_retrack_tracker()?;
        let retrack_create_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });
        let retrack_get_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path(format!("/api/trackers/{}", retrack_tracker.id));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&retrack_tracker);
        });

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_api_tracker(ApiTrackerCreateParams {
                name: "name_one".to_string(),
                enabled: true,
                config: ApiTrackerConfig {
                    revisions: 3,
                    job: None,
                },
                target: ApiTrackerTarget {
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
                notifications: false,
                secrets: Default::default(),
            })
            .await?;
        retrack_create_api_mock.assert();

        let retrack_clear_revisions_api_mock = retrack_server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/api/trackers/{}/revisions", retrack_tracker.id));
            then.status(204).header("Content-Type", "application/json");
        });

        web_scraping.clear_api_tracker_history(tracker.id).await?;

        retrack_create_api_mock.assert();
        retrack_get_api_mock.assert();
        retrack_clear_revisions_api_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_tests_api_request(pool: PgPool) -> anyhow::Result<()> {
        let config = mock_config()?;
        let mock_user = mock_user()?;

        let target_server = MockServer::start();
        let target_mock = target_server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/data");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({"result": "ok"}));
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let result = web_scraping
            .test_api_request(ApiTrackerTestParams {
                target: ApiTrackerTarget {
                    url: format!("{}/api/data", target_server.base_url())
                        .parse()
                        .unwrap(),
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
            })
            .await?;

        assert_eq!(result.status, 200);
        assert!(result.body.contains("result"));
        assert!(result.body.contains("ok"));
        assert!(result.latency_ms < 5000);
        assert_eq!(
            result.headers.get("content-type").map(|s| s.as_str()),
            Some("application/json")
        );

        target_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn properly_tests_api_request_with_post(pool: PgPool) -> anyhow::Result<()> {
        let config = mock_config()?;
        let mock_user = mock_user()?;

        let target_server = MockServer::start();
        let target_mock = target_server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/submit")
                .header("x-custom", "test-value");
            then.status(201)
                .header("Content-Type", "text/plain")
                .body("created");
        });

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let result = web_scraping
            .test_api_request(ApiTrackerTestParams {
                target: ApiTrackerTarget {
                    url: format!("{}/api/submit", target_server.base_url())
                        .parse()
                        .unwrap(),
                    method: Some("POST".to_string()),
                    headers: Some(
                        [("x-custom".to_string(), "test-value".to_string())]
                            .into_iter()
                            .collect(),
                    ),
                    body: Some(serde_json::json!({"key": "value"})),
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
            })
            .await?;

        assert_eq!(result.status, 201);
        assert_eq!(result.body, "created");

        target_mock.assert();

        Ok(())
    }

    #[sqlx::test]
    async fn rejects_test_api_request_with_invalid_url(pool: PgPool) -> anyhow::Result<()> {
        let config = mock_config()?;
        let mock_user = mock_user()?;

        let api = mock_api_with_config(pool, config).await?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let result = web_scraping
            .test_api_request(ApiTrackerTestParams {
                target: ApiTrackerTarget {
                    url: "ftp://not-http.example.com".parse().unwrap(),
                    method: None,
                    headers: None,
                    body: None,
                    media_type: None,
                    accept_statuses: None,
                    accept_invalid_certificates: false,
                    configurator: None,
                    extractor: None,
                },
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("http or https"),
            "Expected URL scheme error, got: {err_msg}"
        );

        Ok(())
    }
}
