mod resources_create_params;
mod resources_get_history_params;
mod resources_update_params;

pub use self::{
    resources_create_params::ResourcesCreateParams,
    resources_get_history_params::ResourcesGetHistoryParams,
    resources_update_params::ResourcesUpdateParams,
};
use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport, IpAddrExt},
    scheduler::SchedulerJob,
    users::UserId,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_scraping::{
            resources::{web_page_resources_revisions_diff, WebScraperResource},
            MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY, MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
        },
        WebPageResource, WebPageResourcesRevision, WebPageResourcesTracker,
        WebScraperResourcesRequest, WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
    },
};
use anyhow::{anyhow, bail};
use async_stream::try_stream;
use cron::Schedule;
use futures::{pin_mut, Stream, StreamExt};
use humantime::format_duration;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

pub struct WebScrapingApiExt<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> WebScrapingApiExt<'a, DR, ET> {
    /// Creates WebScraping API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Returns all web page resources trackers.
    pub async fn get_resources_trackers(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<WebPageResourcesTracker>> {
        self.api
            .db
            .web_scraping(user_id)
            .get_resources_trackers()
            .await
    }

    /// Returns all web page resources tracker job references that have jobs that need to be scheduled.
    pub async fn get_unscheduled_resources_trackers(
        &self,
    ) -> anyhow::Result<Vec<WebPageResourcesTracker>> {
        self.api
            .db
            .web_scraping_system()
            .get_unscheduled_resources_trackers()
            .await
    }

    /// Returns all web page resources trackers that have pending jobs.
    pub fn get_pending_resources_trackers(
        &self,
    ) -> impl Stream<Item = anyhow::Result<WebPageResourcesTracker>> + '_ {
        try_stream! {
            let jobs = self.api.db.get_stopped_scheduler_jobs_by_extra(
                MAX_JOBS_PAGE_SIZE,
                &[SchedulerJob::ResourcesTrackersTrigger as u8],
            );
            pin_mut!(jobs);

            while let Some(job_data) = jobs.next().await {
                let job_id = job_data?
                    .id
                    .ok_or_else(|| anyhow!("Job without ID"))?
                    .into();
                if let Some(tracker) = self.get_resources_tracker_by_job_id(job_id).await? {
                    yield tracker;
                } else {
                    log::error!("Found job ('{job_id}') without corresponding web page resources tracker.");
                }
            }
        }
    }

    /// Returns web page resources tracker by its ID.
    pub async fn get_resources_tracker(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        self.api
            .db
            .web_scraping(user_id)
            .get_resources_tracker(id)
            .await
    }

    /// Returns web page resources tracker by the corresponding job ID.
    pub async fn get_resources_tracker_by_job_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        self.api
            .db
            .web_scraping_system()
            .get_resources_tracker_by_job_id(job_id)
            .await
    }

    /// Creates a new web page resources tracker.
    pub async fn create_resources_tracker(
        &self,
        user_id: UserId,
        params: ResourcesCreateParams,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        let tracker = WebPageResourcesTracker {
            id: Uuid::now_v7(),
            name: params.name,
            url: params.url,
            settings: params.settings,
            user_id,
            job_id: None,
            // Preserve timestamp only up to seconds.
            created_at: OffsetDateTime::from_unix_timestamp(
                OffsetDateTime::now_utc().unix_timestamp(),
            )?,
        };

        self.validate_resources_tracker(&tracker).await?;

        self.api
            .db
            .web_scraping(user_id)
            .insert_resources_tracker(&tracker)
            .await?;

        Ok(tracker)
    }

    /// Updates existing web page resources tracker.
    pub async fn update_resources_tracker(
        &self,
        user_id: UserId,
        id: Uuid,
        params: ResourcesUpdateParams,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        if params.name.is_none() && params.url.is_none() && params.settings.is_none() {
            bail!(SecutilsError::client(format!(
                "Either new name, url, or settings should be provided ({id})."
            )));
        }

        let Some(existing_tracker) = self.get_resources_tracker(user_id, id).await? else {
            bail!(SecutilsError::client(format!(
                "Resources tracker ('{id}') is not found."
            )));
        };

        let changed_url = params
            .url
            .as_ref()
            .map(|url| url != &existing_tracker.url)
            .unwrap_or_default();

        let job_id = match (&params.settings, existing_tracker.job_id) {
            (Some(settings), Some(job_id)) => {
                let changed_schedule = settings.schedule != existing_tracker.settings.schedule;
                if changed_schedule || settings.revisions == 0 {
                    None
                } else {
                    Some(job_id)
                }
            }
            (_, job_id) => job_id,
        };

        let tracker = WebPageResourcesTracker {
            name: params.name.unwrap_or(existing_tracker.name),
            url: params.url.unwrap_or(existing_tracker.url),
            settings: params.settings.unwrap_or(existing_tracker.settings),
            job_id,
            ..existing_tracker
        };

        self.validate_resources_tracker(&tracker).await?;

        let web_scraping = self.api.db.web_scraping(user_id);
        web_scraping.update_resources_tracker(&tracker).await?;

        if changed_url {
            log::debug!(
                "Web resources tracker ('{id}') changed URL, clearing web resources history."
            );
            web_scraping.clear_resources_tracker_history(id).await?;
        }

        Ok(tracker)
    }

    /// Update resources tracker job ID reference (link or unlink).
    pub async fn update_resources_tracker_job(
        &self,
        id: Uuid,
        job_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        self.api
            .db
            .web_scraping_system()
            .update_resources_tracker_job(id, job_id)
            .await
    }

    /// Removes existing web page resources tracker and all history.
    pub async fn remove_resources_tracker(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        self.api
            .db
            .web_scraping(user_id)
            .remove_resources_tracker(id)
            .await
    }

    /// Persists history for the specified web page resources tracker.
    pub async fn create_resources_tracker_revision(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<Option<WebPageResourcesRevision>> {
        // If tracker is configured to persist resource, and client requests refresh, fetch
        // resources with the scraper and persist them.
        // Checks if the specific hostname is a domain and public (not pointing to the local network).
        let is_public_host_name = if let Some(domain) = tracker.url.domain() {
            match self.api.network.resolver.lookup_ip(domain).await {
                Ok(lookup) => lookup.iter().all(|ip| IpAddrExt::is_global(&ip)),
                Err(err) => {
                    log::error!("Cannot resolve `{}` domain to IP: {:?}", domain, err);
                    false
                }
            }
        } else {
            false
        };

        if !is_public_host_name {
            bail!(SecutilsError::client(
                "Resources tracker URL must have a valid public reachable domain name.",
            ));
        }

        let convert_to_web_page_resources =
            |resources: Vec<WebScraperResource>| -> Vec<WebPageResource> {
                resources
                    .into_iter()
                    .map(|resource| resource.into())
                    .collect()
            };

        let scraper_request = WebScraperResourcesRequest::with_default_parameters(&tracker.url)
            .set_delay(tracker.settings.delay)
            .set_scripts(WebScraperResourcesRequestScripts {
                resource_filter_map: tracker.settings.scripts.resource_filter_map.as_deref(),
            });
        let scraper_response = reqwest::Client::new()
            .post(format!(
                "{}api/resources",
                self.api.config.as_ref().components.web_scraper_url.as_str()
            ))
            .json(&scraper_request)
            .send()
            .await?
            .json::<WebScraperResourcesResponse>()
            .await
            .map_err(|err| {
                log::error!(
                    "Cannot fetch resources for `{}` ('{}'): {:?}",
                    tracker.url,
                    tracker.id,
                    err
                );
                anyhow!("Tracker cannot fetch resources due to unexpected error")
            })?;

        // Check if there is a revision with the same timestamp. If so, drop newly fetched revision.
        let web_scraping = self.api.db.web_scraping(user_id);
        let revisions = web_scraping
            .get_resources_tracker_history(tracker.id)
            .await?;
        if revisions
            .iter()
            .any(|revision| revision.created_at == scraper_response.timestamp)
        {
            return Ok(None);
        }

        let new_revision = WebPageResourcesRevision {
            id: Uuid::now_v7(),
            tracker_id: tracker.id,
            scripts: convert_to_web_page_resources(scraper_response.scripts),
            styles: convert_to_web_page_resources(scraper_response.styles),
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
            if !new_revision_with_diff.has_diff() {
                return Ok(None);
            }

            Some(new_revision_with_diff)
        } else {
            None
        };

        // Insert new revision.
        web_scraping
            .insert_resources_tracker_history_revision(&new_revision)
            .await?;

        // Enforce revisions limit and displace old ones.
        if revisions.len() >= tracker.settings.revisions {
            let revisions_to_remove = revisions.len() - tracker.settings.revisions + 1;
            for revision in revisions.iter().take(revisions_to_remove) {
                web_scraping
                    .remove_resources_tracker_history_revision(tracker.id, revision.id)
                    .await?;
            }
        }

        Ok(new_revision_with_diff)
    }

    /// Returns all stored webpage resources tracker history.
    pub async fn get_resources_tracker_history(
        &self,
        user_id: UserId,
        tracker_id: Uuid,
        params: ResourcesGetHistoryParams,
    ) -> anyhow::Result<Vec<WebPageResourcesRevision>> {
        let Some(tracker) = self.get_resources_tracker(user_id, tracker_id).await? else {
            bail!(SecutilsError::client(format!(
                "Resources tracker ('{tracker_id}') is not found."
            )));
        };

        // If tracker is configured to persist resource, and client requests refresh, fetch
        // resources with the scraper and persist them.
        if tracker.settings.revisions > 0 && params.refresh {
            self.create_resources_tracker_revision(user_id, &tracker)
                .await?;
        }

        let revisions = self
            .api
            .db
            .web_scraping(user_id)
            .get_resources_tracker_history(tracker.id)
            .await?;
        if params.calculate_diff {
            web_page_resources_revisions_diff(revisions)
        } else {
            Ok(revisions)
        }
    }

    /// Removes all persisted resources for the specified web page resources tracker.
    pub async fn clear_resources_tracker_history(
        &self,
        user_id: UserId,
        tracker_id: Uuid,
    ) -> anyhow::Result<()> {
        self.api
            .db
            .web_scraping(user_id)
            .clear_resources_tracker_history(tracker_id)
            .await
    }

    async fn validate_resources_tracker(
        &self,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        if tracker.name.is_empty() {
            bail!(SecutilsError::client(
                "Resources tracker name cannot be empty.",
            ));
        }

        if tracker.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
            bail!(SecutilsError::client(format!(
                "Resources tracker name cannot be longer than {} characters.",
                MAX_UTILS_ENTITY_NAME_LENGTH
            )));
        }

        if tracker.settings.revisions > MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS {
            bail!(SecutilsError::client(format!(
                "Resources tracker revisions count cannot be greater than {}.",
                MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS
            )));
        }

        if tracker.settings.delay > MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY {
            bail!(SecutilsError::client(format!(
                "Resources tracker delay cannot be greater than {}ms.",
                MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY.as_millis()
            )));
        }

        if let Some(ref resource_filter) = tracker.settings.scripts.resource_filter_map {
            if resource_filter.is_empty() {
                bail!(SecutilsError::client(
                    "Resources tracker resource filter script cannot be empty."
                ));
            }
        }

        if let Some(schedule) = &tracker.settings.schedule {
            // Validate that the schedule is a valid cron expression.
            let schedule = match Schedule::try_from(schedule.as_str()) {
                Ok(schedule) => schedule,
                Err(err) => {
                    bail!(SecutilsError::client_with_root_cause(
                        anyhow!("Failed to parse schedule `{schedule}`: {err:?}")
                            .context("Resources tracker schedule must be a valid cron expression.")
                    ));
                }
            };

            // Check if the interval between 10 next occurrences is at least 1 hour.
            let next_occurrences = schedule.upcoming(chrono::Utc).take(10).collect::<Vec<_>>();
            let minimum_interval = Duration::from_secs(60 * 60);
            for (index, occurrence) in next_occurrences.iter().enumerate().skip(1) {
                let interval = (*occurrence - next_occurrences[index - 1]).to_std()?;
                if interval < minimum_interval {
                    bail!(SecutilsError::client(format!(
                        "Resources tracker schedule must have at least {} between occurrences, detected {}.",
                        format_duration(minimum_interval),
                        format_duration(interval)
                    )));
                }
            }
        }

        if !self.api.network.is_public_web_url(&tracker.url).await {
            bail!(SecutilsError::client(
                "Resources tracker URL must be either `http` or `https` and have a valid public reachable domain name."
            ));
        }

        Ok(())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data.
    pub fn web_scraping(&self) -> WebScrapingApiExt<DR, ET> {
        WebScrapingApiExt::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        scheduler::SchedulerJob,
        tests::{
            mock_api, mock_api_with_config, mock_api_with_network, mock_config,
            mock_network_with_records, mock_user,
        },
        utils::{
            web_scraping::WebScrapingApiExt, ResourcesCreateParams, ResourcesUpdateParams,
            WebPageResource, WebPageResourceDiffStatus, WebPageResourcesTracker,
            WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings, WebScraperResource,
            WebScraperResourcesRequest, WebScraperResourcesResponse,
        },
    };
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

    #[tokio::test]
    async fn properly_creates_new_web_page_resources_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);

        let tracker = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: None,
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        assert_eq!(
            tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_tracker_at_creation() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);

        let settings = WebPageResourcesTrackerSettings {
            revisions: 3,
            delay: Duration::from_millis(2000),
            enable_notifications: true,
            schedule: None,
            scripts: Default::default(),
        };
        let url = Url::parse("https://secutils.dev")?;

        let create_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty name.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "".to_string(),
                url: url.clone(),
                settings: settings.clone(),
            }).await),
            @r###""Resources tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "a".repeat(101),
                url: url.clone(),
                settings: settings.clone(),
            }).await),
            @r###""Resources tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageResourcesTrackerSettings {
                    revisions: 11,
                    ..settings.clone()
                },
            }).await),
            @r###""Resources tracker revisions count cannot be greater than 10.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageResourcesTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..settings.clone()
                },
            }).await),
            @r###""Resources tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageResourcesTrackerSettings {
                   scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("".to_string()),
                   },
                    ..settings.clone()
                },
            }).await),
            @r###""Resources tracker resource filter script cannot be empty.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageResourcesTrackerSettings {
                   schedule: Some("-".to_string()),
                    ..settings.clone()
                },
            }).await),
            @r###"
        Error {
            context: "Resources tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: url.clone(),
                settings: WebPageResourcesTrackerSettings {
                   schedule: Some("0 * * * * *".to_string()),
                    ..settings.clone()
                },
            }).await),
            @r###""Resources tracker schedule must have at least 1h between occurrences, detected 1m.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            create_and_fail(api.create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: Url::parse("ftp://secutils.dev")?,
                settings: settings.clone(),
            }).await),
            @r###""Resources tracker URL must be either `http` or `https` and have a valid public reachable domain name.""###
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
            create_and_fail(WebScrapingApiExt::new(&api_with_local_network).create_resources_tracker(mock_user.id, ResourcesCreateParams {
                name: "name".to_string(),
                url: Url::parse("https://127.0.0.1")?,
                settings: settings.clone(),
            }).await),
            @r###""Resources tracker URL must be either `http` or `https` and have a valid public reachable domain name.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_web_page_resources_tracker() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);
        let tracker = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: None,
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        // Update name.
        let updated_tracker = api
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageResourcesTracker {
            name: "name_two".to_string(),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        // Update URL.
        let updated_tracker = api
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    url: Some("http://localhost:1234/my/app?q=3".parse()?),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        // Update settings.
        let updated_tracker = api
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    settings: Some(WebPageResourcesTrackerSettings {
                        revisions: 4,
                        enable_notifications: false,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: "http://localhost:1234/my/app?q=3".parse()?,
            settings: WebPageResourcesTrackerSettings {
                revisions: 4,
                enable_notifications: false,
                ..tracker.settings.clone()
            },
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_validates_tracker_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);
        let tracker = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: None,
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let update_and_fail = |result: anyhow::Result<_>| -> SecutilsError {
            result.unwrap_err().downcast::<SecutilsError>().unwrap()
        };

        // Empty parameters.
        let update_result = update_and_fail(
            api.update_resources_tracker(mock_user.id, tracker.id, Default::default())
                .await,
        );
        assert_eq!(
            update_result.to_string(),
            format!(
                "Either new name, url, or settings should be provided ({}).",
                tracker.id
            )
        );

        // Non-existent tracker.
        let update_result = update_and_fail(
            api.update_resources_tracker(
                mock_user.id,
                uuid!("00000000-0000-0000-0000-000000000002"),
                ResourcesUpdateParams {
                    name: Some("name".to_string()),
                    ..Default::default()
                },
            )
            .await,
        );
        assert_eq!(
            update_result.to_string(),
            "Resources tracker ('00000000-0000-0000-0000-000000000002') is not found."
        );

        // Empty name.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                name: Some("".to_string()),
                ..Default::default()
            }).await),
            @r###""Resources tracker name cannot be empty.""###
        );

        // Very long name.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                name: Some("a".repeat(101)),
                ..Default::default()
            }).await),
            @r###""Resources tracker name cannot be longer than 100 characters.""###
        );

        // Too many revisions.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                settings: Some(WebPageResourcesTrackerSettings {
                    revisions: 11,
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Resources tracker revisions count cannot be greater than 10.""###
        );

        // Too long delay.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                settings: Some(WebPageResourcesTrackerSettings {
                    delay: Duration::from_secs(61),
                    ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Resources tracker delay cannot be greater than 60000ms.""###
        );

        // Empty resource filter.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                settings: Some(WebPageResourcesTrackerSettings {
                   scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("".to_string()),
                   },
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Resources tracker resource filter script cannot be empty.""###
        );

        // Invalid schedule.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                settings: Some(WebPageResourcesTrackerSettings {
                   schedule: Some("-".to_string()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###"
        Error {
            context: "Resources tracker schedule must be a valid cron expression.",
            source: "Failed to parse schedule `-`: Error { kind: Expression(\"Invalid cron expression.\") }",
        }
        "###
        );

        // Invalid schedule interval.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                settings: Some(WebPageResourcesTrackerSettings {
                   schedule: Some("0 * * * * *".to_string()),
                   ..tracker.settings.clone()
                }),
                ..Default::default()
            }).await),
            @r###""Resources tracker schedule must have at least 1h between occurrences, detected 1m.""###
        );

        // Invalid URL schema.
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                url: Some(Url::parse("ftp://secutils.dev")?),
                ..Default::default()
            }).await),
            @r###""Resources tracker URL must be either `http` or `https` and have a valid public reachable domain name.""###
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
            .insert_resources_tracker(&tracker)
            .await?;

        // Non-public URL.
        let api = WebScrapingApiExt::new(&api_with_local_network);
        assert_debug_snapshot!(
            update_and_fail(api.update_resources_tracker(mock_user.id, tracker.id, ResourcesUpdateParams {
                url: Some(Url::parse("https://127.0.0.1")?),
                ..Default::default()
            }).await),
            @r###""Resources tracker URL must be either `http` or `https` and have a valid public reachable domain name.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_updates_job_id_at_update() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);
        let tracker = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        // Set job ID.
        api.update_resources_tracker_job(
            tracker.id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        )
        .await?;
        assert_eq!(
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id
        );

        let updated_tracker = api
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    name: Some("name_two".to_string()),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageResourcesTracker {
            name: "name_two".to_string(),
            job_id: Some(uuid!("00000000-0000-0000-0000-000000000001")),
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        // Change in schedule will reset job ID.
        let updated_tracker = api
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    settings: Some(WebPageResourcesTrackerSettings {
                        schedule: Some("0 1 * * * *".to_string()),
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let expected_tracker = WebPageResourcesTracker {
            name: "name_two".to_string(),
            job_id: None,
            settings: WebPageResourcesTrackerSettings {
                schedule: Some("0 1 * * * *".to_string()),
                ..tracker.settings.clone()
            },
            ..tracker.clone()
        };
        assert_eq!(expected_tracker, updated_tracker);
        assert_eq!(
            expected_tracker,
            api.get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_web_page_resources_trackers() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);
        let tracker_one = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        let tracker_two = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_two".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: tracker_one.settings.clone(),
                },
            )
            .await?;

        assert_eq!(
            api.get_resources_trackers(mock_user.id).await?,
            vec![tracker_one.clone(), tracker_two.clone()],
        );

        api.remove_resources_tracker(mock_user.id, tracker_one.id)
            .await?;

        assert_eq!(
            api.get_resources_trackers(mock_user.id).await?,
            vec![tracker_two.clone()],
        );

        api.remove_resources_tracker(mock_user.id, tracker_two.id)
            .await?;

        assert!(api.get_resources_trackers(mock_user.id).await?.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_resources_trackers_by_id() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);

        assert!(api
            .get_resources_tracker(mock_user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .is_none());

        let tracker_one = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        assert_eq!(
            api.get_resources_tracker(mock_user.id, tracker_one.id)
                .await?,
            Some(tracker_one.clone()),
        );

        let tracker_two = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_two".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: tracker_one.settings.clone(),
                },
            )
            .await?;

        assert_eq!(
            api.get_resources_tracker(mock_user.id, tracker_two.id)
                .await?,
            Some(tracker_two.clone()),
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_returns_all_resources_trackers() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let api = WebScrapingApiExt::new(&api);

        assert!(api.get_resources_trackers(mock_user.id).await?.is_empty(),);

        let tracker_one = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        assert_eq!(
            api.get_resources_trackers(mock_user.id).await?,
            vec![tracker_one.clone()],
        );
        let tracker_two = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_two".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: tracker_one.settings.clone(),
                },
            )
            .await?;

        assert_eq!(
            api.get_resources_trackers(mock_user.id).await?,
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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker_one = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        let tracker_two = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_two".to_string(),
                    url: Url::parse("https://secutils.dev/two")?,
                    settings: tracker_one.settings.clone(),
                },
            )
            .await?;

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_two.id, Default::default())
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let mut resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker_one)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();
        resources_mock.delete();

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 1);
        assert_eq!(tracker_one_resources[0].tracker_id, tracker_one.id);
        assert_eq!(
            tracker_one_resources[0].scripts,
            resources_one
                .scripts
                .into_iter()
                .map(WebPageResource::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            tracker_one_resources[0].styles,
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
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker_one)
            .await?
            .unwrap();
        assert_eq!(
            diff.created_at,
            OffsetDateTime::from_unix_timestamp(946720900)?
        );
        assert_eq!(
            diff.scripts,
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
            .get_resources_tracker_history(mock_user.id, tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 2);
        assert!(tracker_two_resources.is_empty());

        let resources_two = get_resources(946720900, "rev_3")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker_two)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();

        let tracker_one_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_one.id, Default::default())
            .await?;
        let tracker_two_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker_two.id, Default::default())
            .await?;
        assert_eq!(tracker_one_resources.len(), 2);
        assert_eq!(tracker_two_resources.len(), 1);

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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert_hits(1);

        let diff = web_scraping
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert_hits(2);

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources_one = get_resources(946720800, "rev_1")?;
        let mut resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();
        resources_mock.delete();

        let resources_two = get_resources(946720900, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        resources_mock.assert();

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        web_scraping
            .clear_resources_tracker_history(mock_user.id, tracker.id)
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        web_scraping
            .remove_resources_tracker(mock_user.id, tracker.id)
            .await?;

        let tracker_resources = api
            .db
            .web_scraping(mock_user.id)
            .get_resources_tracker_history(tracker.id)
            .await?;
        assert!(tracker_resources.is_empty());

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

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        let resources = get_resources(946720800, "rev_1")?;
        let resources_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/api/resources")
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
            .create_resources_tracker_revision(mock_user.id, &tracker)
            .await?;
        assert!(diff.is_none());
        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);
        resources_mock.assert();

        // Update name (resources shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    name: Some("name_one_new".to_string()),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert_eq!(tracker_resources.len(), 1);

        // Update URL.
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    url: Some("https://secutils.dev/two".parse()?),
                    ..Default::default()
                },
            )
            .await?;

        let tracker_resources = web_scraping
            .get_resources_tracker_history(mock_user.id, tracker.id, Default::default())
            .await?;
        assert!(tracker_resources.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn properly_resets_job_id_when_tracker_schedule_changed() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        web_scraping
            .update_resources_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule (job ID shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageResourcesTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        enable_notifications: false,
                        scripts: WebPageResourcesTrackerScripts {
                            resource_filter_map: Some("some".to_string()),
                        },
                        ..tracker.settings.clone()
                    }),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update schedule.
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    settings: Some(WebPageResourcesTrackerSettings {
                        schedule: Some("0 1 * * * *".to_string()),
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id,
            None,
        );

        Ok(())
    }

    #[tokio::test]
    async fn properly_removes_job_id_when_tracker_revisions_disabled() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = WebScrapingApiExt::new(&api);
        let tracker = web_scraping
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev/one")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;
        web_scraping
            .update_resources_tracker_job(
                tracker.id,
                Some(uuid!("00000000-0000-0000-0000-000000000001")),
            )
            .await?;
        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Update everything except schedule and keep revisions enabled (job ID shouldn't be touched).
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    name: Some("name_one_new".to_string()),
                    url: Some(Url::parse("https://secutils.dev/two")?),
                    settings: Some(WebPageResourcesTrackerSettings {
                        revisions: 4,
                        delay: Duration::from_millis(3000),
                        enable_notifications: false,
                        scripts: WebPageResourcesTrackerScripts {
                            resource_filter_map: Some("some".to_string()),
                        },
                        ..tracker.settings.clone()
                    }),
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
                .await?
                .unwrap()
                .job_id,
            Some(uuid!("00000000-0000-0000-0000-000000000001")),
        );

        // Disable revisions.
        web_scraping
            .update_resources_tracker(
                mock_user.id,
                tracker.id,
                ResourcesUpdateParams {
                    settings: Some(WebPageResourcesTrackerSettings {
                        revisions: 0,
                        ..tracker.settings.clone()
                    }),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(
            web_scraping
                .get_resources_tracker(mock_user.id, tracker.id)
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

        let api = WebScrapingApiExt::new(&api);

        let unscheduled_trackers = api.get_unscheduled_resources_trackers().await?;
        assert!(unscheduled_trackers.is_empty());

        let tracker = api
            .create_resources_tracker(
                mock_user.id,
                ResourcesCreateParams {
                    name: "name_one".to_string(),
                    url: Url::parse("https://secutils.dev")?,
                    settings: WebPageResourcesTrackerSettings {
                        revisions: 3,
                        delay: Duration::from_millis(2000),
                        enable_notifications: true,
                        schedule: Some("0 0 * * * *".to_string()),
                        scripts: Default::default(),
                    },
                },
            )
            .await?;

        let unscheduled_trackers = api.get_unscheduled_resources_trackers().await?;
        assert_eq!(unscheduled_trackers, vec![tracker.clone()]);

        api.update_resources_tracker_job(
            tracker.id,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;

        let unscheduled_trackers = api.get_unscheduled_resources_trackers().await?;
        assert!(unscheduled_trackers.is_empty());

        let scheduled_tracker = api
            .get_resources_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert_eq!(
            scheduled_tracker,
            Some(WebPageResourcesTracker {
                job_id: Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
                ..tracker.clone()
            })
        );

        // Remove schedule to make sure that job is removed.
        api.update_resources_tracker(
            mock_user.id,
            tracker.id,
            ResourcesUpdateParams {
                name: None,
                url: None,
                settings: Some(WebPageResourcesTrackerSettings {
                    schedule: None,
                    ..tracker.settings
                }),
            },
        )
        .await?;

        let unscheduled_trackers = api.get_unscheduled_resources_trackers().await?;
        assert!(unscheduled_trackers.is_empty());

        let scheduled_tracker = api
            .get_resources_tracker_by_job_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(scheduled_tracker.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_return_pending_tracker_jobs() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let web_scraping = WebScrapingApiExt::new(&api);

        let pending_trackers = web_scraping
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
                extra: vec![SchedulerJob::ResourcesTrackersTrigger as u8],
                ran: true,
                stopped: n != 1,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: format!("{} 0 0 1 1 * *", n),
                })),
            };

            api.db.upsert_scheduler_job(&job).await?;
        }

        for n in 0..=2 {
            web_scraping
                .create_resources_tracker(
                    mock_user.id,
                    ResourcesCreateParams {
                        name: format!("name_{}", n),
                        url: Url::parse("https://secutils.dev")?,
                        settings: WebPageResourcesTrackerSettings {
                            revisions: 3,
                            delay: Duration::from_millis(2000),
                            enable_notifications: true,
                            schedule: Some("0 0 * * * *".to_string()),
                            scripts: Default::default(),
                        },
                    },
                )
                .await?;
        }

        let pending_trackers = web_scraping
            .get_pending_resources_trackers()
            .collect::<Vec<_>>()
            .await;
        assert!(pending_trackers.is_empty());

        // Assign job IDs to trackers.
        let all_trackers = web_scraping.get_resources_trackers(mock_user.id).await?;
        for (n, tracker) in all_trackers.iter().enumerate() {
            web_scraping
                .update_resources_tracker_job(
                    tracker.id,
                    Some(uuid::Uuid::parse_str(&format!(
                        "67e55044-10b1-426f-9247-bb680e5fe0c{}",
                        n
                    ))?),
                )
                .await?;
        }

        let pending_trackers = web_scraping
            .get_pending_resources_trackers()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>, _>>()?;
        assert_eq!(pending_trackers.len(), 2);

        let all_trackers = web_scraping.get_resources_trackers(mock_user.id).await?;
        assert_eq!(
            vec![all_trackers[0].clone(), all_trackers[2].clone()],
            pending_trackers,
        );

        let all_trackers = web_scraping.get_resources_trackers(mock_user.id).await?;
        assert_eq!(all_trackers.len(), 3);

        Ok(())
    }
}
