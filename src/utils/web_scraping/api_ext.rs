mod web_page_content_tracker_get_history_params;
mod web_page_resources_tracker_get_history_params;
mod web_page_tracker_create_params;
mod web_page_tracker_update_params;

pub use self::{
    web_page_content_tracker_get_history_params::WebPageContentTrackerGetHistoryParams,
    web_page_resources_tracker_get_history_params::WebPageResourcesTrackerGetHistoryParams,
    web_page_tracker_create_params::WebPageTrackerCreateParams,
    web_page_tracker_update_params::WebPageTrackerUpdateParams,
};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    scheduler::{ScheduleExt, SchedulerJobRetryStrategy},
    users::User,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_scraping::{
            database_ext::WebScrapingDatabaseSystemExt, web_page_content_revisions_diff,
            web_page_resources_revisions_diff, WebPageContentTrackerTag, WebPageDataRevision,
            WebPageResource, WebPageResourcesData, WebPageResourcesTrackerInternalTag,
            WebPageResourcesTrackerTag, WebPageTracker, WebPageTrackerTag,
            WebScraperContentRequest, WebScraperContentRequestScripts, WebScraperContentResponse,
            WebScraperErrorResponse, WebScraperResource, WebScraperResourcesRequest,
            WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
        },
    },
};
use anyhow::{anyhow, bail};
use cron::Schedule;
use futures::Stream;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

/// Script used to `filter_map` resource that needs to be tracked.
pub const WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME: &str = "resourceFilterMap";

/// Script used to extract web page content that needs to be tracked.
pub const WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME: &str = "extractContent";

/// We currently wait up to 60 seconds before starting to track web page.
const MAX_WEB_PAGE_TRACKER_DELAY: Duration = Duration::from_secs(60);

/// We currently support up to 10 retry attempts for the web page tracker.
const MAX_WEB_PAGE_TRACKER_RETRY_ATTEMPTS: u32 = 10;

/// We currently support minimum 60 seconds between retry attempts for the web page tracker.
const MIN_WEB_PAGE_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// We currently support maximum 12 hours between retry attempts for the web page tracker.
const MAX_WEB_PAGE_TRACKER_RETRY_INTERVAL: Duration = Duration::from_secs(12 * 3600);

pub struct WebScrapingApiExt<'a, 'u, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
    user: &'u User,
}

impl<'a, 'u, DR: DnsResolver, ET: EmailTransport> WebScrapingApiExt<'a, 'u, DR, ET> {
    /// Creates WebScraping API.
    pub fn new(api: &'a Api<DR, ET>, user: &'u User) -> Self {
        Self { api, user }
    }

    /// Returns all web page resources trackers.
    pub async fn get_resources_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<WebPageResourcesTrackerTag>>> {
        self.get_web_page_trackers().await
    }

    /// Returns all web page content trackers.
    pub async fn get_content_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<WebPageContentTrackerTag>>> {
        self.get_web_page_trackers().await
    }

    /// Returns web page resources tracker by its ID.
    pub async fn get_resources_tracker(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<WebPageResourcesTrackerTag>>> {
        self.get_web_page_tracker(id).await
    }

    /// Returns web page content tracker by its ID.
    #[allow(dead_code)]
    pub async fn get_content_tracker(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<WebPageContentTrackerTag>>> {
        self.get_web_page_tracker(id).await
    }

    /// Creates a new web page resources tracker.
    pub async fn create_resources_tracker(
        &self,
        params: WebPageTrackerCreateParams,
    ) -> anyhow::Result<WebPageTracker<WebPageResourcesTrackerTag>> {
        self.create_web_page_tracker(
            params,
            Some(|tracker: &WebPageTracker<WebPageResourcesTrackerTag>| {
                self.validate_web_page_resources_tracker(tracker)
            }),
        )
        .await
    }

    /// Creates a new web page content tracker.
    pub async fn create_content_tracker(
        &self,
        params: WebPageTrackerCreateParams,
    ) -> anyhow::Result<WebPageTracker<WebPageContentTrackerTag>> {
        self.create_web_page_tracker(
            params,
            Some(|tracker: &WebPageTracker<WebPageContentTrackerTag>| {
                self.validate_web_page_content_tracker(tracker)
            }),
        )
        .await
    }

    /// Updates existing web page resources tracker.
    pub async fn update_resources_tracker(
        &self,
        id: Uuid,
        params: WebPageTrackerUpdateParams,
    ) -> anyhow::Result<WebPageTracker<WebPageResourcesTrackerTag>> {
        self.update_web_page_tracker(
            id,
            params,
            Some(|tracker: &WebPageTracker<WebPageResourcesTrackerTag>| {
                self.validate_web_page_resources_tracker(tracker)
            }),
        )
        .await
    }

    /// Updates existing web page content tracker.
    pub async fn update_content_tracker(
        &self,
        id: Uuid,
        params: WebPageTrackerUpdateParams,
    ) -> anyhow::Result<WebPageTracker<WebPageContentTrackerTag>> {
        self.update_web_page_tracker(
            id,
            params,
            Some(|tracker: &WebPageTracker<WebPageContentTrackerTag>| {
                self.validate_web_page_content_tracker(tracker)
            }),
        )
        .await
    }

    /// Removes existing web page resources tracker and all history.
    pub async fn remove_web_page_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .web_scraping(self.user.id)
            .remove_web_page_tracker(id)
            .await
    }

    /// Persists history for the specified web page resources tracker.
    pub async fn create_resources_tracker_revision(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Option<WebPageDataRevision<WebPageResourcesTrackerTag>>> {
        let Some(tracker) = self.get_resources_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "Web page tracker ('{tracker_id}') is not found."
            )));
        };

        let features = self.user.subscription.get_features(&self.api.config);
        let max_revisions = std::cmp::min(
            tracker.settings.revisions,
            features.config.web_scraping.tracker_revisions,
        );
        if max_revisions == 0 {
            return Ok(None);
        }

        let convert_to_web_page_resources =
            |resources: Vec<WebScraperResource>| -> Vec<WebPageResource> {
                resources
                    .into_iter()
                    .map(|resource| resource.into())
                    .collect()
            };

        let scraper_request = WebScraperResourcesRequest::with_default_parameters(&tracker.url)
            .set_delay(tracker.settings.delay);
        let resources_filter_map_script = tracker
            .settings
            .scripts
            .as_ref()
            .and_then(|scripts| scripts.get(WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME));
        let scraper_request = if let Some(resources_filter_map) = resources_filter_map_script {
            scraper_request.set_scripts(WebScraperResourcesRequestScripts {
                resource_filter_map: Some(resources_filter_map),
            })
        } else {
            scraper_request
        };

        let scraper_request = if let Some(headers) = tracker.settings.headers.as_ref() {
            scraper_request.set_headers(headers)
        } else {
            scraper_request
        };

        let scraper_response = reqwest::Client::new()
            .post(format!(
                "{}api/web_page/resources",
                self.api.config.as_ref().components.web_scraper_url.as_str()
            ))
            .json(&scraper_request)
            .send()
            .await
            .map_err(|err| {
                anyhow!(
                    "Could not connect to the web scraper service to extract resources for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;

        if !scraper_response.status().is_success() {
            let is_client_error = scraper_response.status().is_client_error();
            let scraper_error_response = scraper_response
                .json::<WebScraperErrorResponse>()
                .await
                .map_err(|err| {
                anyhow!(
                    "Could not deserialize scraper error response for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;
            if is_client_error {
                bail!(SecutilsError::client(scraper_error_response.message));
            } else {
                bail!(
                    "Unexpected scraper error for the web tracker ('{}'): {:?}",
                    tracker.id,
                    scraper_error_response.message
                );
            }
        }

        let scraper_response = scraper_response
            .json::<WebScraperResourcesResponse>()
            .await
            .map_err(|err| {
                anyhow!(
                    "Could not deserialize scraper response for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;

        // Check if there is a revision with the same timestamp. If so, drop newly fetched revision.
        let web_scraping = self.api.db.web_scraping(self.user.id);
        let revisions = web_scraping
            .get_web_page_tracker_history(tracker.id)
            .await?;
        if revisions
            .iter()
            .any(|revision| revision.created_at == scraper_response.timestamp)
        {
            return Ok(None);
        }

        let new_revision = WebPageDataRevision {
            id: Uuid::now_v7(),
            tracker_id: tracker.id,
            data: WebPageResourcesData {
                scripts: convert_to_web_page_resources(scraper_response.scripts),
                styles: convert_to_web_page_resources(scraper_response.styles),
            },
            created_at: scraper_response.timestamp,
        };

        // Get the latest revision and check if it's different from the new one. If so, we need to
        // save a new revision, otherwise drop it.
        let new_revision_with_diff = if let Some(latest_revision) = revisions.last() {
            let mut revisions_with_diff = web_page_resources_revisions_diff(vec![
                latest_revision.clone(),
                new_revision.clone(),
            ])?;
            let new_revision_with_diff = revisions_with_diff
                .pop()
                .ok_or_else(|| anyhow!("Invalid revisions diff result."))?;

            // Return the latest revision back to the queue if it's different from the new one.
            if !new_revision_with_diff.data.has_diff() {
                return Ok(None);
            }

            Some(new_revision_with_diff)
        } else {
            None
        };

        // Insert new revision.
        web_scraping
            .insert_web_page_tracker_history_revision::<WebPageResourcesTrackerInternalTag>(
                &WebPageDataRevision {
                    id: new_revision.id,
                    tracker_id: new_revision.tracker_id,
                    data: WebPageResourcesData {
                        scripts: new_revision
                            .data
                            .scripts
                            .into_iter()
                            .map(Into::into)
                            .collect(),
                        styles: new_revision
                            .data
                            .styles
                            .into_iter()
                            .map(Into::into)
                            .collect(),
                    },
                    created_at: new_revision.created_at,
                },
            )
            .await?;

        // Enforce revisions limit and displace old ones.
        if revisions.len() >= max_revisions {
            let revisions_to_remove = revisions.len() - max_revisions + 1;
            for revision in revisions.iter().take(revisions_to_remove) {
                web_scraping
                    .remove_web_page_tracker_history_revision(tracker.id, revision.id)
                    .await?;
            }
        }

        Ok(new_revision_with_diff)
    }

    /// Persists history for the specified web page content tracker.
    pub async fn create_content_tracker_revision(
        &self,
        tracker_id: Uuid,
    ) -> anyhow::Result<Option<WebPageDataRevision<WebPageContentTrackerTag>>> {
        let Some(tracker) = self.get_content_tracker(tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "Web page tracker ('{tracker_id}') is not found."
            )));
        };

        // Enforce revisions limit and displace old ones.
        let features = self.user.subscription.get_features(&self.api.config);
        let max_revisions = std::cmp::min(
            tracker.settings.revisions,
            features.config.web_scraping.tracker_revisions,
        );
        if max_revisions == 0 {
            return Ok(None);
        }

        let web_scraping = self.api.db.web_scraping(self.user.id);
        let revisions = web_scraping
            .get_web_page_tracker_history::<WebPageContentTrackerTag>(tracker.id)
            .await?;

        let scraper_request = WebScraperContentRequest::with_default_parameters(&tracker.url)
            .set_delay(tracker.settings.delay);
        let scraper_request = if let Some(revision) = revisions.last() {
            scraper_request.set_previous_content(&revision.data)
        } else {
            scraper_request
        };

        let extract_content_script = tracker
            .settings
            .scripts
            .as_ref()
            .and_then(|scripts| scripts.get(WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME));
        let scraper_request = if let Some(extract_content) = extract_content_script {
            scraper_request.set_scripts(WebScraperContentRequestScripts {
                extract_content: Some(extract_content),
            })
        } else {
            scraper_request
        };

        let scraper_request = if let Some(headers) = tracker.settings.headers.as_ref() {
            scraper_request.set_headers(headers)
        } else {
            scraper_request
        };

        let scraper_response = reqwest::Client::new()
            .post(format!(
                "{}api/web_page/content",
                self.api.config.as_ref().components.web_scraper_url.as_str()
            ))
            .json(&scraper_request)
            .send()
            .await
            .map_err(|err| {
                anyhow!(
                    "Could not connect to the web scraper service to extract content for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;

        if !scraper_response.status().is_success() {
            let is_client_error = scraper_response.status().is_client_error();
            let scraper_error_response = scraper_response
                .json::<WebScraperErrorResponse>()
                .await
                .map_err(|err| {
                anyhow!(
                    "Could not deserialize scraper error response for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;
            if is_client_error {
                bail!(SecutilsError::client(scraper_error_response.message));
            } else {
                bail!(
                    "Unexpected scraper error for the web tracker ('{}'): {:?}",
                    tracker.id,
                    scraper_error_response.message
                );
            }
        }

        let scraper_response = scraper_response
            .json::<WebScraperContentResponse>()
            .await
            .map_err(|err| {
                anyhow!(
                    "Could not deserialize scraper response for the web tracker ('{}'): {:?}",
                    tracker.id,
                    err
                )
            })?;

        // Check if there is a revision with the same timestamp. If so, drop newly fetched revision.
        if revisions
            .iter()
            .any(|revision| revision.created_at == scraper_response.timestamp)
        {
            return Ok(None);
        }

        // Check if content has changed.
        if let Some(revision) = revisions.last() {
            if revision.data == scraper_response.content {
                return Ok(None);
            }
        }

        let new_revision = WebPageDataRevision {
            id: Uuid::now_v7(),
            tracker_id: tracker.id,
            data: scraper_response.content,
            created_at: scraper_response.timestamp,
        };

        // Insert new revision.
        web_scraping
            .insert_web_page_tracker_history_revision::<WebPageContentTrackerTag>(&new_revision)
            .await?;

        // Enforce revisions limit and displace old ones.
        if revisions.len() >= max_revisions {
            let revisions_to_remove = revisions.len() - max_revisions + 1;
            for revision in revisions.iter().take(revisions_to_remove) {
                web_scraping
                    .remove_web_page_tracker_history_revision(tracker.id, revision.id)
                    .await?;
            }
        }

        Ok(Some(new_revision))
    }

    /// Returns all stored webpage resources tracker history.
    pub async fn get_resources_tracker_history(
        &self,
        tracker_id: Uuid,
        params: WebPageResourcesTrackerGetHistoryParams,
    ) -> anyhow::Result<Vec<WebPageDataRevision<WebPageResourcesTrackerTag>>> {
        if params.refresh {
            self.create_resources_tracker_revision(tracker_id).await?;
        } else if self.get_resources_tracker(tracker_id).await?.is_none() {
            bail!(SecutilsError::client(format!(
                "Web page tracker ('{tracker_id}') is not found."
            )));
        }

        let revisions = self
            .api
            .db
            .web_scraping(self.user.id)
            .get_web_page_tracker_history::<WebPageResourcesTrackerInternalTag>(tracker_id)
            .await?
            .into_iter()
            .map(|revision| WebPageDataRevision {
                id: revision.id,
                tracker_id: revision.tracker_id,
                data: WebPageResourcesData {
                    scripts: revision.data.scripts.into_iter().map(Into::into).collect(),
                    styles: revision.data.styles.into_iter().map(Into::into).collect(),
                },
                created_at: revision.created_at,
            })
            .collect::<Vec<_>>();
        if params.calculate_diff {
            web_page_resources_revisions_diff(revisions)
        } else {
            Ok(revisions)
        }
    }

    /// Returns all stored webpage content tracker history.
    pub async fn get_content_tracker_history(
        &self,
        tracker_id: Uuid,
        params: WebPageContentTrackerGetHistoryParams,
    ) -> anyhow::Result<Vec<WebPageDataRevision<WebPageContentTrackerTag>>> {
        if params.refresh {
            self.create_content_tracker_revision(tracker_id).await?;
        } else if self.get_content_tracker(tracker_id).await?.is_none() {
            bail!(SecutilsError::client(format!(
                "Web page tracker ('{tracker_id}') is not found."
            )));
        }

        let revisions = self
            .api
            .db
            .web_scraping(self.user.id)
            .get_web_page_tracker_history::<WebPageContentTrackerTag>(tracker_id)
            .await?;
        if params.calculate_diff {
            web_page_content_revisions_diff(revisions)
        } else {
            Ok(revisions)
        }
    }

    /// Removes all persisted resources for the specified web page resources tracker.
    pub async fn clear_web_page_tracker_history(&self, tracker_id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .web_scraping(self.user.id)
            .clear_web_page_tracker_history(tracker_id)
            .await
    }

    /// Returns all web page trackers.
    async fn get_web_page_trackers<Tag: WebPageTrackerTag>(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<Tag>>> {
        self.api
            .db
            .web_scraping(self.user.id)
            .get_web_page_trackers()
            .await
    }

    /// Returns web page tracker by its ID.
    async fn get_web_page_tracker<Tag: WebPageTrackerTag>(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<Tag>>> {
        self.api
            .db
            .web_scraping(self.user.id)
            .get_web_page_tracker(id)
            .await
    }

    /// Creates a new web page tracker.
    async fn create_web_page_tracker<Tag: WebPageTrackerTag, V>(
        &self,
        params: WebPageTrackerCreateParams,
        validator: Option<V>,
    ) -> anyhow::Result<WebPageTracker<Tag>>
    where
        V: Fn(&WebPageTracker<Tag>) -> anyhow::Result<()>,
    {
        let tracker = WebPageTracker {
            id: Uuid::now_v7(),
            name: params.name,
            url: params.url,
            settings: params.settings,
            user_id: self.user.id,
            job_id: None,
            job_config: params.job_config,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
            meta: None,
        };

        self.validate_web_page_tracker(&tracker).await?;
        // Run custom validator if specified.
        if let Some(validator) = validator {
            validator(&tracker)?;
        }

        self.api
            .db
            .web_scraping(self.user.id)
            .insert_web_page_tracker(&tracker)
            .await?;

        Ok(tracker)
    }

    /// Updates existing web page tracker.
    async fn update_web_page_tracker<Tag: WebPageTrackerTag, V>(
        &self,
        id: Uuid,
        params: WebPageTrackerUpdateParams,
        validator: Option<V>,
    ) -> anyhow::Result<WebPageTracker<Tag>>
    where
        V: Fn(&WebPageTracker<Tag>) -> anyhow::Result<()>,
    {
        if params.name.is_none()
            && params.url.is_none()
            && params.settings.is_none()
            && params.job_config.is_none()
        {
            bail!(SecutilsError::client(format!(
                "Either new name, url, settings, or job config should be provided ({id})."
            )));
        }

        let Some(existing_tracker) = self
            .api
            .db
            .web_scraping(self.user.id)
            .get_web_page_tracker(id)
            .await?
        else {
            bail!(SecutilsError::client(format!(
                "Web page tracker ('{id}') is not found."
            )));
        };

        let changed_url = params
            .url
            .as_ref()
            .map(|url| url != &existing_tracker.url)
            .unwrap_or_default();

        let disabled_revisions = params
            .settings
            .as_ref()
            .map(|settings| settings.revisions == 0)
            .unwrap_or_default();

        let changed_schedule = params
            .job_config
            .as_ref()
            .map(
                |job_config| match (&existing_tracker.job_config, job_config) {
                    (Some(existing_job_config), Some(job_config)) => {
                        job_config.schedule != existing_job_config.schedule
                    }
                    _ => true,
                },
            )
            .unwrap_or_default();

        let job_id = if disabled_revisions || changed_schedule {
            None
        } else {
            existing_tracker.job_id
        };

        let tracker = WebPageTracker {
            name: params.name.unwrap_or(existing_tracker.name),
            url: params.url.unwrap_or(existing_tracker.url),
            settings: params.settings.unwrap_or(existing_tracker.settings),
            job_id,
            job_config: params.job_config.unwrap_or(existing_tracker.job_config),
            ..existing_tracker
        };

        self.validate_web_page_tracker(&tracker).await?;
        if let Some(validator) = validator {
            validator(&tracker)?;
        }

        let web_scraping = self.api.db.web_scraping(self.user.id);
        web_scraping.update_web_page_tracker(&tracker).await?;

        if changed_url {
            log::debug!("Web page tracker ('{id}') changed URL, clearing web resources history.");
            web_scraping.clear_web_page_tracker_history(id).await?;
        }

        Ok(tracker)
    }

    async fn validate_web_page_tracker<Tag: WebPageTrackerTag>(
        &self,
        tracker: &WebPageTracker<Tag>,
    ) -> anyhow::Result<()> {
        if tracker.name.is_empty() {
            bail!(SecutilsError::client(
                "Web page tracker name cannot be empty.",
            ));
        }

        if tracker.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Web page tracker name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        let features = self.user.subscription.get_features(&self.api.config);
        if tracker.settings.revisions > features.config.web_scraping.tracker_revisions {
            bail!(SecutilsError::client(format!(
                "Web page tracker revisions count cannot be greater than {}.",
                features.config.web_scraping.tracker_revisions
            )));
        }

        if tracker.settings.delay > MAX_WEB_PAGE_TRACKER_DELAY {
            bail!(SecutilsError::client(format!(
                "Web page tracker delay cannot be greater than {}ms.",
                MAX_WEB_PAGE_TRACKER_DELAY.as_millis()
            )));
        }

        if let Some(ref scripts) = tracker.settings.scripts {
            if scripts
                .iter()
                .any(|(name, script)| name.is_empty() || script.is_empty())
            {
                bail!(SecutilsError::client(
                    "Web page tracker scripts cannot be empty or have an empty name."
                ));
            }
        }

        if let Some(job_config) = &tracker.job_config {
            // Validate that the schedule is a valid cron expression.
            let schedule = match Schedule::try_from(job_config.schedule.as_str()) {
                Ok(schedule) => schedule,
                Err(err) => {
                    bail!(SecutilsError::client_with_root_cause(
                        anyhow!(
                            "Failed to parse schedule `{}`: {err:?}",
                            job_config.schedule
                        )
                        .context("Web page tracker schedule must be a valid cron expression.")
                    ));
                }
            };

            // Check if the interval between 10 next occurrences is at least 1 hour.
            let min_schedule_interval = schedule.min_interval()?;
            let min_allowed_interval = Duration::from_secs(60 * 60);
            if min_schedule_interval < min_allowed_interval {
                bail!(SecutilsError::client(format!(
                    "Web page tracker schedule must have at least {} between occurrences, but detected {}.",
                    humantime::format_duration(min_allowed_interval),
                    humantime::format_duration(min_schedule_interval)
                )));
            }

            // Validate retry strategy.
            if let Some(retry_strategy) = &job_config.retry_strategy {
                let max_attempts = retry_strategy.max_attempts();
                if max_attempts == 0 || max_attempts > MAX_WEB_PAGE_TRACKER_RETRY_ATTEMPTS {
                    bail!(SecutilsError::client(
                        format!("Web page tracker max retry attempts cannot be zero or greater than {MAX_WEB_PAGE_TRACKER_RETRY_ATTEMPTS}, but received {max_attempts}.")
                    ));
                }

                let min_interval = *retry_strategy.min_interval();
                if min_interval < MIN_WEB_PAGE_TRACKER_RETRY_INTERVAL {
                    bail!(SecutilsError::client(
                        format!(
                            "Web page tracker min retry interval cannot be less than {}, but received {}.",
                            humantime::format_duration(MIN_WEB_PAGE_TRACKER_RETRY_INTERVAL),
                            humantime::format_duration(min_interval)
                        )
                    ));
                }

                if let SchedulerJobRetryStrategy::Linear { max_interval, .. }
                | SchedulerJobRetryStrategy::Exponential { max_interval, .. } = retry_strategy
                {
                    let max_interval = *max_interval;
                    if max_interval < MIN_WEB_PAGE_TRACKER_RETRY_INTERVAL {
                        bail!(SecutilsError::client(
                            format!(
                                "Web page tracker retry strategy max interval cannot be less than {}, but received {}.",
                                humantime::format_duration(MIN_WEB_PAGE_TRACKER_RETRY_INTERVAL),
                                humantime::format_duration(max_interval)
                            )
                        ));
                    }

                    if max_interval > MAX_WEB_PAGE_TRACKER_RETRY_INTERVAL
                        || max_interval > min_schedule_interval
                    {
                        bail!(SecutilsError::client(
                            format!(
                                "Web page tracker retry strategy max interval cannot be greater than {}, but received {}.",
                                humantime::format_duration(MAX_WEB_PAGE_TRACKER_RETRY_INTERVAL.min(min_schedule_interval)),
                                humantime::format_duration(max_interval)
                            )
                        ));
                    }
                }
            }
        }

        if !self.api.network.is_public_web_url(&tracker.url).await {
            bail!(SecutilsError::client(
                format!("Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received {}.", tracker.url)
            ));
        }

        Ok(())
    }

    fn validate_web_page_resources_tracker(
        &self,
        tracker: &WebPageTracker<WebPageResourcesTrackerTag>,
    ) -> anyhow::Result<()> {
        if let Some(ref scripts) = tracker.settings.scripts {
            if !scripts.is_empty()
                && !scripts.contains_key(WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME)
            {
                bail!(SecutilsError::client(
                    "Web page tracker contains unrecognized scripts."
                ));
            }
        }

        Ok(())
    }

    fn validate_web_page_content_tracker(
        &self,
        tracker: &WebPageTracker<WebPageContentTrackerTag>,
    ) -> anyhow::Result<()> {
        if let Some(ref scripts) = tracker.settings.scripts {
            if !scripts.is_empty()
                && !scripts.contains_key(WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME)
            {
                bail!(SecutilsError::client(
                    "Web page tracker contains unrecognized scripts."
                ));
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

pub struct WebScrapingSystemApiExt<'a> {
    web_scraping_system: WebScrapingDatabaseSystemExt<'a>,
}

impl<'a> WebScrapingSystemApiExt<'a> {
    /// Creates WebScraping System API.
    pub fn new<DR: DnsResolver, ET: EmailTransport>(api: &'a Api<DR, ET>) -> Self {
        Self {
            web_scraping_system: api.db.web_scraping_system(),
        }
    }

    /// Returns all web page resources tracker job references that have jobs that need to be scheduled.
    pub async fn get_unscheduled_resources_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<WebPageResourcesTrackerTag>>> {
        self.get_unscheduled_web_page_trackers().await
    }

    /// Returns all web page content tracker job references that have jobs that need to be scheduled.
    pub async fn get_unscheduled_content_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<WebPageContentTrackerTag>>> {
        self.get_unscheduled_web_page_trackers().await
    }

    /// Returns all web page resources trackers that have pending jobs.
    pub fn get_pending_resources_trackers(
        &self,
    ) -> impl Stream<Item = anyhow::Result<WebPageTracker<WebPageResourcesTrackerTag>>> + '_ {
        self.web_scraping_system
            .get_pending_web_page_trackers(MAX_JOBS_PAGE_SIZE)
    }

    /// Returns all web page resources trackers that have pending jobs.
    pub fn get_pending_content_trackers(
        &self,
    ) -> impl Stream<Item = anyhow::Result<WebPageTracker<WebPageContentTrackerTag>>> + '_ {
        self.web_scraping_system
            .get_pending_web_page_trackers(MAX_JOBS_PAGE_SIZE)
    }

    /// Returns web page resources tracker by the corresponding job ID.
    pub async fn get_resources_tracker_by_job_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<WebPageResourcesTrackerTag>>> {
        self.get_web_page_tracker_by_job_id(job_id).await
    }

    /// Returns web page content tracker by the corresponding job ID.
    pub async fn get_content_tracker_by_job_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<WebPageContentTrackerTag>>> {
        self.get_web_page_tracker_by_job_id(job_id).await
    }

    /// Update resources tracker job ID reference (link or unlink).
    pub async fn update_web_page_tracker_job(
        &self,
        id: Uuid,
        job_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        self.web_scraping_system
            .update_web_page_tracker_job(id, job_id)
            .await
    }

    /// Returns all web page tracker job references that have jobs that need to be scheduled.
    async fn get_unscheduled_web_page_trackers<Tag: WebPageTrackerTag>(
        &self,
    ) -> anyhow::Result<Vec<WebPageTracker<Tag>>> {
        self.web_scraping_system
            .get_unscheduled_web_page_trackers()
            .await
    }

    /// Returns web page tracker by the corresponding job ID.
    async fn get_web_page_tracker_by_job_id<Tag: WebPageTrackerTag>(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageTracker<Tag>>> {
        self.web_scraping_system
            .get_web_page_tracker_by_job_id(job_id)
            .await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data (as system user).
    pub fn web_scraping_system(&self) -> WebScrapingSystemApiExt {
        WebScrapingSystemApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        scheduler::{
            SchedulerJob, SchedulerJobConfig, SchedulerJobMetadata, SchedulerJobRetryStrategy,
        },
        tests::{
            mock_api, mock_api_with_config, mock_api_with_network, mock_config,
            mock_network_with_records, mock_user,
        },
        utils::web_scraping::{
            api_ext::{
                WebPageContentTrackerGetHistoryParams, WebPageResourcesTrackerGetHistoryParams,
                WebPageTrackerUpdateParams,
            },
            tests::{
                WebPageTrackerCreateParams, WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME,
                WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
            },
            WebPageContentTrackerTag, WebPageResource, WebPageResourceDiffStatus,
            WebPageResourcesTrackerTag, WebPageTracker, WebPageTrackerKind, WebPageTrackerSettings,
            WebScraperContentRequest, WebScraperContentResponse, WebScraperErrorResponse,
            WebScraperResource, WebScraperResourcesRequest, WebScraperResourcesResponse,
        },
    };
    use actix_web::ResponseError;
    use futures::StreamExt;
    use httpmock::MockServer;
    use insta::assert_debug_snapshot;
    use std::{net::Ipv4Addr, time::Duration};
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{CronJob, JobStored, JobStoredData, JobType};
    use trust_dns_resolver::{
        proto::rr::{rdata::A, RData, Record},
        Name,
    };
    use url::Url;
    use uuid::uuid;

    fn get_resources(timestamp: i64, label: &str) -> anyhow::Result<WebScraperResourcesResponse> {
        Ok(WebScraperResourcesResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(timestamp)?,
            scripts: vec![WebScraperResource {
                url: Some(Url::parse(&format!(
                    "http://localhost:1234/script_{label}.js"
                ))?),
                content: None,
            }],
            styles: vec![WebScraperResource {
                url: Some(Url::parse(&format!(
                    "http://localhost:1234/style_{label}.css"
                ))?),
                content: None,
            }],
        })
    }

    fn get_content(timestamp: i64, label: &str) -> anyhow::Result<WebScraperContentResponse> {
        Ok(WebScraperContentResponse {
            timestamp: OffsetDateTime::from_unix_timestamp(timestamp)?,
            content: label.to_string(),
        })
    }

    #[tokio::test]
    async fn properly_creates_new_web_page_resources_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraper = api.web_scraping(&mock_user);

        let tracker = web_scraper
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            })
            .await?;

        assert_eq!(
            tracker,
            web_scraper
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_creates_new_web_page_content_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);

        let tracker = api
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            })
            .await?;

        assert_eq!(tracker, api.get_content_tracker(tracker.id).await?.unwrap());

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_web_page_resources_tracker_at_creation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            headers: Default::default(),
        };
        let url = Url::parse("https://secutils.dev")?;

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "a".repeat(101),
                url: url.clone(),
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    revisions: 31,
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker revisions count cannot be greater than 30.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    scripts: Some([(
                        WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker scripts cannot be empty or have an empty name.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    scripts: Some([(
                        "someScript".to_string(),
                            "return resource;".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker contains unrecognized scripts.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "-".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
            }).await),
            @r###"
        Error {
            context: "Web page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 * * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker schedule must have at least 1h between occurrences, but detected 1m.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 0,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 11,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@monthly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(13 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(2 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            create_and_fail(web_scraping.create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: Url::parse("ftp://secutils.dev")?,
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received ftp://secutils.dev/.""###
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;

        // Non-public URL.
        assert_debug_snapshot!(
            create_and_fail(api_with_local_network.web_scraping(&mock_user).create_resources_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: Url::parse("https://127.0.0.1")?,
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received https://127.0.0.1/.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_web_page_content_tracker_at_creation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);

        let settings = WebPageTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2000),
            scripts: Default::default(),
            headers: Default::default(),
        };
        let url = Url::parse("https://secutils.dev")?;

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "a".repeat(101),
                url: url.clone(),
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    revisions: 31,
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker revisions count cannot be greater than 30.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    scripts: Some([(
                        WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME.to_string(),
                            "".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker scripts cannot be empty or have an empty name.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageTrackerSettings {
                    scripts: Some([(
                        "someScript".to_string(),
                            "return resource;".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    ..settings.clone()
                },
                job_config: None
            }).await),
            @r###""Web page tracker contains unrecognized scripts.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "-".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
            }).await),
            @r###"
        Error {
            context: "Web page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "0 * * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker schedule must have at least 1h between occurrences, but detected 1m.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 0,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 11,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@monthly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(13 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: settings.clone(),
                job_config: Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(2 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            create_and_fail(api.create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: Url::parse("ftp://secutils.dev")?,
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received ftp://secutils.dev/.""###
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;

        // Non-public URL.
        assert_debug_snapshot!(
            create_and_fail(api_with_local_network.web_scraping(&mock_user).create_content_tracker(WebPageTrackerCreateParams {
                name: "name".to_string(),
                url: Url::parse("https://127.0.0.1")?,
                settings: settings.clone(),
                job_config: None
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received https://127.0.0.1/.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_web_page_resources_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: None,
            })
            .await?;

        // Update name.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        // Update URL.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    url: Some("http://localhost:1234/my/app?q=3".parse()?),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        // Update settings.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        // Update job config.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            job_config: Some(SchedulerJobConfig {
                schedule: "@hourly".to_string(),
                retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 5,
                }),
                notifications: false,
            }),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        // Remove job config.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            job_config: None,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_web_page_resources_tracker_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);
        let tracker = api
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
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            })
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            api.update_resources_tracker(tracker.id, Default::default())
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, url, settings, or job config should be provided ({}).",
                tracker.id
            )
        );

        // Non-existent tracker.
        let update_result = update_and_fail(
            api.update_resources_tracker(
                uuid!("00000000-0000-0000-0000-000000000002"),
                WebPageTrackerUpdateParams {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Web page tracker ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                name: Some("".to_string()),
                ..Default::default()
            }).await),
            @r###""Web page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                name: Some("a".repeat(101)),
                ..Default::default()
            }).await),
            @r###""Web page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    revisions: 31,
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker revisions count cannot be greater than 30.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    scripts: Some([(
                        WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                        "".to_string()
                    )]
                    .into_iter()
                    .collect()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker scripts cannot be empty or have an empty name.""###
        );

        // Unknown script.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    scripts: Some([(
                        "someScript".to_string(),
                        "return resource;".to_string()
                    )]
                    .into_iter()
                    .collect()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker contains unrecognized scripts.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "-".to_string(),
                    retry_strategy: None,
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###"
        Error {
            context: "Web page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "0 * * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker schedule must have at least 1h between occurrences, but detected 1m.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 0,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 11,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
           update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@monthly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(13 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(2 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
               ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                url: Some(Url::parse("ftp://secutils.dev")?),
                ..Default::default()
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received ftp://secutils.dev/.""###
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;
        api_with_local_network.db.insert_user(&mock_user).await?;
        api_with_local_network
            .db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker(&tracker)
            .await?;

        // Non-public URL.
        let web_scraping = api_with_local_network.web_scraping(&mock_user);
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_resources_tracker(tracker.id, WebPageTrackerUpdateParams {
                url: Some(Url::parse("https://127.0.0.1")?),
                ..Default::default()
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received https://127.0.0.1/.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_web_page_content_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = api.web_scraping(&mock_user);
        let tracker = api
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                settings: WebPageTrackerSettings {
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    headers: Default::default(),
                },
                job_config: None,
            })
            .await?;

        // Update name.
        let updated_tracker = api
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_content_tracker(tracker.id).await?.unwrap()
        );

        // Update URL.
        let updated_tracker = api
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    url: Some("http://localhost:1234/my/app?q=3".parse()?),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_content_tracker(tracker.id).await?.unwrap()
        );

        // Update settings.
        let updated_tracker = api
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_content_tracker(tracker.id).await?.unwrap()
        );

        // Update job config.
        let updated_tracker = api
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "@hourly".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            job_config: Some(SchedulerJobConfig {
                schedule: "@hourly".to_string(),
                retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                    interval: Duration::from_secs(120),
                    max_attempts: 5,
                }),
                notifications: false,
            }),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_content_tracker(tracker.id).await?.unwrap()
        );

        // Remove job config.
        let updated_tracker = api
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageTrackerSettings {
                revisions: 4,
                ..tracker.settings.clone()
            },
            job_config: None,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_content_tracker(tracker.id).await?.unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_web_page_content_tracker_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
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
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 5,
                    }),
                    notifications: false,
                }),
            })
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            web_scraping
                .update_content_tracker(tracker.id, Default::default())
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, url, settings, or job config should be provided ({}).",
                tracker.id
            )
        );

        // Non-existent tracker.
        let update_result = update_and_fail(
            web_scraping
                .update_content_tracker(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    WebPageTrackerUpdateParams {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Web page tracker ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                name: Some("".to_string()),
                ..Default::default()
            }).await),
            @r###""Web page tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                name: Some("a".repeat(101)),
                ..Default::default()
            }).await),
            @r###""Web page tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    revisions: 31,
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker revisions count cannot be greater than 30.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    scripts: Some([(
                        WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME.to_string(),
                        "".to_string()
                    )]
                    .into_iter()
                    .collect()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker scripts cannot be empty or have an empty name.""###
        );

        // Unknown script.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                settings: Some(WebPageTrackerSettings {
                    scripts: Some([(
                        "someScript".to_string(),
                        "return resource;".to_string()
                    )]
                    .into_iter()
                    .collect()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Web page tracker contains unrecognized scripts.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "-".to_string(),
                    retry_strategy: None,
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###"
        Error {
            context: "Web page tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "0 * * * * *".to_string(),
                    retry_strategy: None,
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker schedule must have at least 1h between occurrences, but detected 1m.""###
        );

        // Too few retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 0,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 0.""###
        );

        // Too many retry attempts.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(120),
                        max_attempts: 11,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker max retry attempts cannot be zero or greater than 10, but received 11.""###
        );

        // Too low retry interval.
        assert_debug_snapshot!(
           update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                        interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker min retry interval cannot be less than 1m, but received 30s.""###
        );

        // Too low max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@daily".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(30),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be less than 1m, but received 30s.""###
        );

        // Too high max retry interval.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@monthly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(13 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
                ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 12h, but received 13h.""###
        );

        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                job_config: Some(Some(SchedulerJobConfig {
                    schedule: "@hourly".to_string(),
                    retry_strategy: Some(SchedulerJobRetryStrategy::Linear {
                        initial_interval: Duration::from_secs(120),
                        increment: Duration::from_secs(10),
                        max_interval: Duration::from_secs(2 * 3600),
                        max_attempts: 5,
                    }),
                    notifications: false,
                })),
               ..Default::default()
            }).await),
            @r###""Web page tracker retry strategy max interval cannot be greater than 1h, but received 2h.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                url: Some(Url::parse("ftp://secutils.dev")?),
                ..Default::default()
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received ftp://secutils.dev/.""###
        );

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;
        api_with_local_network.db.insert_user(&mock_user).await?;
        api_with_local_network
            .db
            .web_scraping(mock_user.id)
            .insert_web_page_tracker(&tracker)
            .await?;

        // Non-public URL.
        let web_scraping = api_with_local_network.web_scraping(&mock_user);
        assert_debug_snapshot!(
            update_and_fail(web_scraping.update_content_tracker(tracker.id, WebPageTrackerUpdateParams {
                url: Some(Url::parse("https://127.0.0.1")?),
                ..Default::default()
            }).await),
            @r###""Web page tracker URL must be either `http` or `https` and have a valid public reachable domain name, but received https://127.0.0.1/.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_web_page_resources_job_id_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
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

        // Set job ID.
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id
        );

        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            job_id: Some(uuid!("00000000-0000-0000-0000-000000000001")),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        // Change in schedule will reset job ID.
        let updated_tracker = web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    })),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            job_id: None,
            job_config: Some(SchedulerJobConfig {
                schedule: "0 1 * * * *".to_string(),
                retry_strategy: None,
                notifications: true,
            }),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_web_page_content_job_id_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
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

        // Set job ID.
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id
        );

        let updated_tracker = web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            job_id: Some(uuid!("00000000-0000-0000-0000-000000000001")),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_content_tracker(tracker.id).await?.unwrap()
        );

        // Change in schedule will reset job ID.
        let updated_tracker = web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: None,
                        notifications: true,
                    })),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageTracker {
            name: "name_two".to_string(),
            job_id: None,
            job_config: Some(SchedulerJobConfig {
                schedule: "0 1 * * * *".to_string(),
                retry_strategy: None,
                notifications: true,
            }),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            web_scraping.get_content_tracker(tracker.id).await?.unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_trackers() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker_one = web_scraping
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
        let tracker_two = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        assert_eq!(
            web_scraping.get_resources_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );

        web_scraping.remove_web_page_tracker(tracker_one.id).await?;

        assert_eq!(
            web_scraping.get_resources_trackers().await?,
            vec![tracker_two.clone()],
        );

        web_scraping.remove_web_page_tracker(tracker_two.id).await?;

        assert!(web_scraping.get_resources_trackers().await?.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_resources_trackers_by_id() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(web_scraping
            .get_resources_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .is_none());

        let tracker_one = web_scraping
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
        assert_eq!(
            web_scraping.get_resources_tracker(tracker_one.id).await?,
            Some(tracker_one.clone()),
        );

        let tracker_two = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        assert_eq!(
            web_scraping.get_resources_tracker(tracker_two.id).await?,
            Some(tracker_two.clone()),
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_content_trackers_by_id() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(web_scraping
            .get_content_tracker(uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .is_none());

        let tracker_one = web_scraping
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
        assert_eq!(
            web_scraping.get_content_tracker(tracker_one.id).await?,
            Some(tracker_one.clone()),
        );

        let tracker_two = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        assert_eq!(
            web_scraping.get_content_tracker(tracker_two.id).await?,
            Some(tracker_two.clone()),
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_all_resources_trackers() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(web_scraping.get_resources_trackers().await?.is_empty(),);

        let tracker_one = web_scraping
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
        assert_eq!(
            web_scraping.get_resources_trackers().await?,
            vec![tracker_one.clone()],
        );
        let tracker_two = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        assert_eq!(
            web_scraping.get_resources_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_all_content_trackers() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        assert!(web_scraping.get_content_trackers().await?.is_empty(),);

        let tracker_one = web_scraping
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
        assert_eq!(
            web_scraping.get_content_trackers().await?,
            vec![tracker_one.clone()],
        );
        let tracker_two = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        assert_eq!(
            web_scraping.get_content_trackers().await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_saves_web_page_resources() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker_one = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        let tracker_two = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev/two")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let mut resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker_one.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_one);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker_one.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();
        resources_mock.delete();

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 1);
        assert_eq!(tracker_one_resources[0].tracker_id, tracker_one.id);
        assert_eq!(
            tracker_one_resources[0].data.scripts,
            resources_one
                .scripts
                .into_iter()
                .map(WebPageResource::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            tracker_one_resources[0].data.styles,
            resources_one
                .styles
                .into_iter()
                .map(WebPageResource::from)
                .collect::<Vec<_>>()
        );
        assert!(tracker_two_resources.is_empty());

        let resources_two = get_resources(946720900, "rev_2")?;
        let mut resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker_one.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_two);
        });
        let diff = web_scraping
            .create_resources_tracker_revision(tracker_one.id)
            .await?
            .unwrap();
        assert_eq!(
            diff.created_at,
            OffsetDateTime::from_unix_timestamp(946720900)?
        );
        assert_eq!(
            diff.data.scripts,
            vec![
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/script_rev_2.js")?),
                    content: None,
                    diff_status: Some(WebPageResourceDiffStatus::Added),
                },
                WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/script_rev_1.js")?),
                    content: None,
                    diff_status: Some(WebPageResourceDiffStatus::Removed),
                },
            ]
        );
        resources_mock.assert();
        resources_mock.delete();

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 2);
        assert!(tracker_two_resources.is_empty());

        let resources_two = get_resources(946720900, "rev_3")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker_two.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_two);
        });
        let diff = web_scraping
            .create_resources_tracker_revision(tracker_two.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 2);
        assert_eq!(tracker_two_resources.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_forwards_error_if_web_page_resources_extraction_fails() -> anyhow::Result<()>
    {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let web_scraper_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body_obj(&WebScraperErrorResponse {
                    message: "some client-error".to_string(),
                });
        });

        let scraper_error = web_scraping
            .get_resources_tracker_history(
                tracker.id,
                WebPageResourcesTrackerGetHistoryParams {
                    refresh: true,
                    calculate_diff: false,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(scraper_error.status_code(), 400);
        assert_debug_snapshot!(
            scraper_error,
            @r###""some client-error""###
        );

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());
        web_scraper_mock.assert();

        Ok(())
    }

    #[tokio::test]
    async fn properly_saves_web_page_content() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker_one = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        let tracker_two = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_two".to_string(),
                url: Url::parse("https://secutils.dev/two")?,
                settings: tracker_one.settings.clone(),
                job_config: tracker_one.job_config.clone(),
            })
            .await?;

        let tracker_one_history = web_scraping
            .get_content_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_history = web_scraping
            .get_content_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert!(tracker_one_history.is_empty());
        assert!(tracker_two_history.is_empty());

        let content_one = get_content(946720800, "\"rev_1\"")?;
        let mut content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker_one.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_one);
        });

        let tracker_one_history = web_scraping
            .get_content_tracker_history(
                tracker_one.id,
                WebPageContentTrackerGetHistoryParams {
                    refresh: true,
                    calculate_diff: false,
                },
            )
            .await?;
        let tracker_two_content = web_scraping
            .get_content_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_history.len(), 1);
        assert_eq!(tracker_one_history[0].tracker_id, tracker_one.id);
        assert_eq!(tracker_one_history[0].data, content_one.content);
        assert!(tracker_two_content.is_empty());

        content_mock.assert();
        content_mock.delete();

        let content_two = get_content(946720900, "\"rev_2\"")?;
        let mut content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker_one.url)
                            .set_delay(Duration::from_millis(2000))
                            .set_previous_content("\"rev_1\""),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_two);
        });
        let revision = web_scraping
            .create_content_tracker_revision(tracker_one.id)
            .await?
            .unwrap();
        assert_eq!(
            revision.created_at,
            OffsetDateTime::from_unix_timestamp(946720900)?
        );
        assert_eq!(revision.data, "\"rev_2\"");
        content_mock.assert();
        content_mock.delete();

        let tracker_one_content = web_scraping
            .get_content_tracker_history(tracker_one.id, Default::default())
            .await?;
        let tracker_two_content = web_scraping
            .get_content_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_content.len(), 2);
        assert!(tracker_two_content.is_empty());

        let content_two = get_content(946720900, "\"rev_3\"")?;
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker_two.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_two);
        });
        let revision = web_scraping
            .create_content_tracker_revision(tracker_two.id)
            .await?;
        assert_eq!(revision.unwrap().data, "\"rev_3\"");
        content_mock.assert();

        let tracker_one_content = web_scraping
            .get_content_tracker_history(
                tracker_one.id,
                WebPageContentTrackerGetHistoryParams {
                    refresh: false,
                    calculate_diff: true,
                },
            )
            .await?;
        let tracker_two_content = web_scraping
            .get_content_tracker_history(tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_content.len(), 2);
        assert_eq!(tracker_two_content.len(), 1);

        assert_debug_snapshot!(
            tracker_one_content.into_iter().map(|rev| rev.data).collect::<Vec<_>>(),
            @r###"
        [
            "\"rev_1\"",
            "@@ -1 +1 @@\n-rev_1\n+rev_2\n",
        ]
        "###
        );
        assert_debug_snapshot!(
            tracker_two_content.into_iter().map(|rev| rev.data).collect::<Vec<_>>(),
            @r###"
        [
            "\"rev_3\"",
        ]
        "###
        );

        let tracker_one_content = web_scraping
            .get_content_tracker_history(
                tracker_one.id,
                WebPageContentTrackerGetHistoryParams {
                    refresh: false,
                    calculate_diff: false,
                },
            )
            .await?;
        assert_debug_snapshot!(
            tracker_one_content.into_iter().map(|rev| rev.data).collect::<Vec<_>>(),
            @r###"
        [
            "\"rev_1\"",
            "\"rev_2\"",
        ]
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_forwards_error_if_web_page_content_extraction_fails() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body_obj(&WebScraperErrorResponse {
                    message: "some client-error".to_string(),
                });
        });

        let scraper_error = web_scraping
            .get_content_tracker_history(
                tracker.id,
                WebPageContentTrackerGetHistoryParams {
                    refresh: true,
                    calculate_diff: false,
                },
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_eq!(scraper_error.status_code(), 400);
        assert_debug_snapshot!(
            scraper_error,
            @r###""some client-error""###
        );

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());
        content_mock.assert();

        Ok(())
    }

    #[tokio::test]
    async fn properly_ignores_web_page_resources_with_the_same_timestamp() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_one);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert_hits(1);

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert_hits(2);

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_ignores_web_page_content_with_the_same_timestamp() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        let content_one = get_content(946720800, "\"rev_1\"")?;
        let mut content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_one);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert_eq!(revision.unwrap().data, "\"rev_1\"");
        content_mock.assert_hits(1);

        content_mock.delete();
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000))
                            .set_previous_content("\"rev_1\""),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_one);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert!(revision.is_none());
        content_mock.assert_hits(1);

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_ignores_web_page_resources_with_no_diff() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let mut resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_one);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();
        resources_mock.delete();

        let resources_two = get_resources(946720900, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources_two);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_ignores_web_page_content_with_no_diff() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        let content_one = get_content(946720800, "\"rev_1\"")?;
        let mut content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_one);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert_eq!(revision.unwrap().data, "\"rev_1\"");
        content_mock.assert();
        content_mock.delete();

        let content_two = get_content(946720900, "\"rev_1\"")?;
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000))
                            .set_previous_content("\"rev_1\""),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content_two);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert!(revision.is_none());
        content_mock.assert();

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_resources() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        web_scraping
            .clear_web_page_tracker_history(tracker.id)
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_content() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        let content = get_content(946720800, "\"rev_1\"")?;
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert!(revision.is_some());
        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);
        content_mock.assert();

        web_scraping
            .clear_web_page_tracker_history(tracker.id)
            .await?;

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_resources_when_tracker_is_removed() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        web_scraping.remove_web_page_tracker(tracker.id).await?;

        let tracker_resources = api
            .db
            .web_scraping(mock_user.id)
            .get_web_page_tracker_history::<WebPageResourcesTrackerTag>(tracker.id)
            .await?;
        assert!(tracker_resources.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_content_when_tracker_is_removed() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        let content = get_content(946720800, "\"rev_1\"")?;
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert!(revision.is_some());
        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);
        content_mock.assert();

        web_scraping.remove_web_page_tracker(tracker.id).await?;

        let tracker_content = api
            .db
            .web_scraping(mock_user.id)
            .get_web_page_tracker_history::<WebPageContentTrackerTag>(tracker.id)
            .await?;
        assert!(tracker_content.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_resources_when_tracker_url_changed() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/resources")
                .json_body(
                    serde_json::to_value(
                        WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&resources);
        });

        let diff = web_scraping
            .create_resources_tracker_revision(tracker.id)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        // Update name (resources shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

        // Update URL.
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    url: Some("https://secutils.dev/two".parse()?),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_content_when_tracker_url_changed() -> anyhow::Result<()> {
        let server = MockServer::start();
        let mut config = mock_config()?;
        config.components.web_scraper_url = Url::parse(&server.base_url())?;

        let api = mock_api_with_config(config).await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        let content = get_content(946720800, "\"rev_1\"")?;
        let content_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/web_page/content")
                .json_body(
                    serde_json::to_value(
                        WebScraperContentRequest::with_default_parameters(&tracker.url)
                            .set_delay(Duration::from_millis(2000)),
                    )
                    .unwrap(),
                );
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body_obj(&content);
        });

        let revision = web_scraping
            .create_content_tracker_revision(tracker.id)
            .await?;
        assert!(revision.is_some());
        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);
        content_mock.assert();

        // Update name (content shouldn't be touched).
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_content.len(), 1);

        // Update URL.
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    url: Some("https://secutils.dev/two".parse()?),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_content = web_scraping
            .get_content_tracker_history(tracker.id, Default::default())
            .await?;
        assert!(tracker_content.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_resets_web_page_resources_job_id_when_tracker_schedule_changed(
    ) -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule (job ID shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        scripts: Some(
                            [(
                                WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                                "some".to_string(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        ..tracker.settings.clone()
                    }),
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update schedule.
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None,
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_resets_web_page_content_job_id_when_tracker_schedule_changed(
    ) -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule (job ID shouldn't be touched).
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        scripts: Some(
                            [(
                                WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME.to_string(),
                                "some".to_string(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        ..tracker.settings.clone()
                    }),
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update schedule.
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 1 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None,
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_resources_job_id_when_tracker_revisions_disabled(
    ) -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_resources_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule and keep revisions enabled (job ID shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        scripts: Some(
                            [(
                                WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                                "some".to_string(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        ..tracker.settings.clone()
                    }),
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Disable revisions.
        web_scraping
            .update_resources_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    settings: Some(WebPageTrackerSettings {
                        revisions: 0,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None,
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_content_job_id_when_tracker_revisions_disabled(
    ) -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);
        let tracker = web_scraping
            .create_content_tracker(WebPageTrackerCreateParams {
                name: "name_one".to_string(),
                url: Url::parse("https://secutils.dev/one")?,
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
        api.web_scraping_system()
            .update_web_page_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule and keep revisions enabled (job ID shouldn't be touched).
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        scripts: Some(
                            [(
                                WEB_PAGE_CONTENT_TRACKER_EXTRACT_SCRIPT_NAME.to_string(),
                                "some".to_string(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        ..tracker.settings.clone()
                    }),
                    job_config: Some(Some(SchedulerJobConfig {
                        schedule: "0 0 * * * *".to_string(),
                        retry_strategy: Some(SchedulerJobRetryStrategy::Constant {
                            interval: Duration::from_secs(120),
                            max_attempts: 5,
                        }),
                        notifications: false,
                    })),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Disable revisions.
        web_scraping
            .update_content_tracker(
                tracker.id,
                WebPageTrackerUpdateParams {
                    settings: Some(WebPageTrackerSettings {
                        revisions: 0,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_content_tracker(tracker.id)
                .await?
                .unwrap()
                .job_id,
            None,
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_manipulate_tracker_jobs() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());
        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_content_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

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

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert_eq!(unscheduled_trackers, vec![resources_tracker.clone()]);
        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_content_trackers()
            .await?;
        assert_eq!(unscheduled_trackers, vec![content_tracker.clone()]);

        api.web_scraping_system()
            .update_web_page_tracker_job(
                resources_tracker.id,
                Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
            )
            .await?;
        api.web_scraping_system()
            .update_web_page_tracker_job(
                content_tracker.id,
                Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c9")),
            )
            .await?;

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());
        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_content_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        let scheduled_tracker = api
            .web_scraping_system()
            .get_resources_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert_eq!(
            scheduled_tracker,
            Some(WebPageTracker {
                job_id: Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
                ..resources_tracker.clone()
            })
        );

        let scheduled_tracker = api
            .web_scraping_system()
            .get_content_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c9"))
            .await?;
        assert_eq!(
            scheduled_tracker,
            Some(WebPageTracker {
                job_id: Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c9")),
                ..content_tracker.clone()
            })
        );

        // Remove schedule to make sure that job is removed.
        web_scraping
            .update_resources_tracker(
                resources_tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        web_scraping
            .update_content_tracker(
                content_tracker.id,
                WebPageTrackerUpdateParams {
                    job_config: Some(None),
                    ..Default::default()
                },
            )
            .await?;

        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());
        let unscheduled_trackers = api
            .web_scraping_system()
            .get_unscheduled_content_trackers()
            .await?;
        assert!(unscheduled_trackers.is_empty());

        let scheduled_tracker = api
            .web_scraping_system()
            .get_resources_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(scheduled_tracker.is_none());

        let scheduled_tracker = api
            .web_scraping_system()
            .get_content_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c9"))
            .await?;
        assert!(scheduled_tracker.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_return_pending_resources_tracker_jobs() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_resources_trackers()
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        for n in 0..=2 {
            let job = JobStoredData {
                id: Some(
                    uuid::Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?
                        .into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: (SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageResources,
                }))
                .try_into()?,
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
                time_offset_seconds: 0,
            };

            api.db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            web_scraping
                .create_resources_tracker(WebPageTrackerCreateParams {
                    name: format!("name_{}", n),
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
        }

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_resources_trackers()
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        // Assign job IDs to trackers.
        let all_trackers = web_scraping.get_resources_trackers().await?;
        for (n, tracker) in all_trackers.iter().enumerate() {
            api.web_scraping_system()
                .update_web_page_tracker_job(
                    tracker.id,
                    Some(uuid::Uuid::parse_str(&format!(
                        "67e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?),
                )
                .await?;
        }

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_resources_trackers()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>, _>>()?;
        assert_eq!(pending_trackers.len(), 2);

        let all_trackers = web_scraping.get_resources_trackers().await?;
        assert_eq!(
            vec![all_trackers[0].clone(), all_trackers[2].clone()],
            pending_trackers,
        );

        let all_trackers = web_scraping.get_resources_trackers().await?;
        assert_eq!(all_trackers.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn can_return_pending_content_tracker_jobs() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = api.web_scraping(&mock_user);

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_content_trackers()
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        for n in 0..=2 {
            let job = JobStoredData {
                id: Some(
                    uuid::Uuid::parse_str(&format!("67e55044-10b1-426f-9247-bb680e5fe0c{}", n))?
                        .into(),
                ),
                last_updated: Some(946720800u64 + n),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: n as u32,
                job_type: JobType::Cron as i32,
                extra: (SchedulerJobMetadata::new(SchedulerJob::WebPageTrackersTrigger {
                    kind: WebPageTrackerKind::WebPageContent,
                }))
                .try_into()?,
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
                time_offset_seconds: 0,
            };

            api.db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            web_scraping
                .create_content_tracker(WebPageTrackerCreateParams {
                    name: format!("name_{}", n),
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
        }

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_content_trackers()
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        // Assign job IDs to trackers.
        let all_trackers = web_scraping.get_content_trackers().await?;
        for (n, tracker) in all_trackers.iter().enumerate() {
            api.web_scraping_system()
                .update_web_page_tracker_job(
                    tracker.id,
                    Some(uuid::Uuid::parse_str(&format!(
                        "67e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?),
                )
                .await?;
        }

        let pending_trackers = api
            .web_scraping_system()
            .get_pending_content_trackers()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>, _>>()?;
        assert_eq!(pending_trackers.len(), 2);

        let all_trackers = web_scraping.get_content_trackers().await?;
        assert_eq!(
            vec![all_trackers[0].clone(), all_trackers[2].clone()],
            pending_trackers,
        );

        let all_trackers = web_scraping.get_content_trackers().await?;
        assert_eq!(all_trackers.len(), 3);

        Ok(())
    }
}
