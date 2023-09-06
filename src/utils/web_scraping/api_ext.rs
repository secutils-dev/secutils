use crate::{
    api::Api,
    config::Config,
    database::Database,
    network::{DnsResolver, EmailTransport, IpAddrExt, Network},
    scheduler::SchedulerJob,
    users::{
        DictionaryDataUserDataSetter, InternalUserDataNamespace, PublicUserDataNamespace, UserData,
        UserDataKey, UserDataNamespace, UserId,
    },
    utils::{
        web_scraping::resources::{web_page_resources_revisions_diff, WebScraperResource},
        WebPageResource, WebPageResourcesRevision, WebPageResourcesTracker,
        WebScraperResourcesRequest, WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
    },
};
use anyhow::{anyhow, bail};
use async_stream::try_stream;
use futures::{pin_mut, Stream, StreamExt};
use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};
use time::OffsetDateTime;
use uuid::Uuid;

/// Defines a maximum number of jobs that can be retrieved from the database at once.
const MAX_JOBS_PAGE_SIZE: usize = 1000;

pub struct WebScrapingApi<'a, C: AsRef<Config>, DR: DnsResolver, ET: EmailTransport> {
    config: C,
    db: Cow<'a, Database>,
    network: &'a Network<DR, ET>,
}

impl<'a, C: AsRef<Config>, DR: DnsResolver, ET: EmailTransport> WebScrapingApi<'a, C, DR, ET> {
    /// Creates WebScraping API.
    pub fn new(config: C, db: &'a Database, network: &'a Network<DR, ET>) -> Self {
        Self {
            config,
            db: Cow::Borrowed(db),
            network,
        }
    }

    /// Updates existing or creates new web page resources tracker.
    pub async fn upsert_resources_tracker(
        &self,
        user_id: UserId,
        tracker: WebPageResourcesTracker,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        // First retrieve tracker and check if it has different URL. If so, we need to remove all
        // tracked resources.
        let existing_tracker = self.get_resources_tracker(user_id, &tracker.name).await?;
        let (changed_tracking, changed_url) = if let Some(existing_tracker) = &existing_tracker {
            (
                existing_tracker.schedule != tracker.schedule
                    || existing_tracker.revisions != tracker.revisions,
                existing_tracker.url != tracker.url,
            )
        } else {
            (false, false)
        };

        if changed_url {
            log::debug!(
                "Web resources tracker \"{}\" (user ID: {:?}) changed URL, clearing web resources history.",
                tracker.name,
                user_id
            );
            let user_data_key = (
                PublicUserDataNamespace::WebPageResourcesTrackers,
                tracker.name.as_str(),
            );
            self.db.remove_user_data(user_id, user_data_key).await?;
        }

        let should_track = tracker.schedule.is_some() && tracker.revisions > 0;
        if !should_track && changed_tracking {
            self.remove_resources_tracker_job(user_id, &tracker.name)
                .await?;
        } else if should_track && (changed_tracking || existing_tracker.is_none()) {
            self.upsert_resources_tracker_job(user_id, &tracker.name, None)
                .await?;
        }

        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::WebPageResourcesTrackers,
            UserData::new(
                user_id,
                [(tracker.name.clone(), Some(tracker.clone()))]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await?;

        Ok(tracker)
    }

    /// Removes existing web page resources tracker.
    pub async fn remove_resources_tracker(
        &self,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<()> {
        // First delete tracked resources and jobs.
        self.db
            .remove_user_data(
                user_id,
                (
                    PublicUserDataNamespace::WebPageResourcesTrackers,
                    tracker_name,
                ),
            )
            .await?;
        self.remove_resources_tracker_job(user_id, tracker_name)
            .await?;

        // Then delete the tracker itself.
        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::WebPageResourcesTrackers,
            UserData::new(
                user_id,
                [(tracker_name.to_string(), None)]
                    .into_iter()
                    .collect::<BTreeMap<_, Option<WebPageResourcesTracker>>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await
    }

    /// Returns web page resources tracker by its name.
    pub async fn get_resources_tracker(
        &self,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        Ok(self
            .db
            .get_user_data::<BTreeMap<String, WebPageResourcesTracker>>(
                user_id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .and_then(|mut map| map.value.remove(tracker_name)))
    }

    /// Returns all stored webpage resources for the specified web page resources tracker.
    pub async fn get_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<Vec<WebPageResourcesRevision>> {
        Ok(self
            .db
            .get_user_data::<Vec<WebPageResourcesRevision>>(
                user_id,
                (
                    PublicUserDataNamespace::WebPageResourcesTrackers,
                    tracker.name.as_str(),
                ),
            )
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default())
    }

    /// Persists resources for the specified web page resources tracker.
    pub async fn save_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
        new_revision: WebPageResourcesRevision,
    ) -> anyhow::Result<()> {
        let user_data_key = UserDataKey::from((
            PublicUserDataNamespace::WebPageResourcesTrackers,
            tracker.name.as_str(),
        ));

        let mut revisions = self
            .db
            .get_user_data::<VecDeque<WebPageResourcesRevision>>(user_id, user_data_key)
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default();

        // Check if there is a revision with the same timestamp. If so, we need to replace it.
        if let Some(position) = revisions
            .iter()
            .position(|r| r.timestamp == new_revision.timestamp)
        {
            revisions[position] = new_revision;
        } else {
            // Get the latest revision and check if it's different from the new one. If so, we need to
            // save a new revision, otherwise just replace the latest.
            let new_revision = if let Some(latest_revision) = revisions.pop_back() {
                let revisions_with_diff = web_page_resources_revisions_diff(vec![
                    latest_revision.clone(),
                    new_revision.clone(),
                ])?;
                let new_revision_with_diff = revisions_with_diff
                    .get(1)
                    .ok_or_else(|| anyhow!("Invalid revisions diff result."))?;

                // Return the latest revision back to the queue if it's different from the new one.
                if new_revision_with_diff.has_diff() {
                    revisions.push_back(latest_revision);
                }

                new_revision
            } else {
                new_revision
            };

            // Enforce revisions limit and displace the oldest one.
            if revisions.len() == tracker.revisions {
                revisions.pop_front();
            }
            revisions.push_back(new_revision);
        }

        self.db
            .upsert_user_data(
                user_data_key,
                UserData::new_with_key(
                    user_id,
                    &tracker.name,
                    revisions,
                    OffsetDateTime::now_utc(),
                ),
            )
            .await
    }

    /// Fetches resources for the specified web page resources tracker.
    pub async fn fetch_resources(
        &self,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<WebPageResourcesRevision> {
        // If tracker is configured to persist resource, and client requests refresh, fetch
        // resources with the scraper and persist them.
        // Checks if the specific hostname is a domain and public (not pointing to the local network).
        let is_public_host_name = if let Some(domain) = tracker.url.domain() {
            match self.network.resolver.lookup_ip(domain).await {
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
            bail!("Tracker URL must have a valid public reachable domain name");
        }

        let convert_to_web_page_resources =
            |resources: Vec<WebScraperResource>| -> Vec<WebPageResource> {
                resources
                    .into_iter()
                    .map(|resource| resource.into())
                    .collect()
            };

        let scraper_request = WebScraperResourcesRequest::with_default_parameters(&tracker.url)
            .set_delay(tracker.delay)
            .set_scripts(WebScraperResourcesRequestScripts {
                resource_filter: tracker.scripts.resource_filter.as_deref(),
            });
        let scraper_response = reqwest::Client::new()
            .post(format!(
                "{}api/resources",
                self.config.as_ref().components.web_scraper_url.as_str()
            ))
            .json(&scraper_request)
            .send()
            .await?
            .json::<WebScraperResourcesResponse>()
            .await
            .map_err(|err| {
                log::error!(
                    "Cannot fetch resources for `{}` ({}): {:?}",
                    tracker.url,
                    tracker.name,
                    err
                );
                anyhow!("Tracker cannot fetch resources due to unexpected error")
            })?;

        Ok(WebPageResourcesRevision {
            timestamp: scraper_response.timestamp,
            scripts: convert_to_web_page_resources(scraper_response.scripts),
            styles: convert_to_web_page_resources(scraper_response.styles),
        })
    }

    /// Removes all persisted resources for the specified web page resources tracker.
    pub async fn remove_tracked_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        self.db
            .remove_user_data(
                user_id,
                (
                    PublicUserDataNamespace::WebPageResourcesTrackers,
                    tracker.name.as_str(),
                ),
            )
            .await
    }

    /// Returns all web page resources tracker job references that have jobs that need to be scheduled.
    pub async fn get_unscheduled_resources_tracker_jobs(
        &self,
    ) -> anyhow::Result<Vec<UserData<Option<Uuid>>>> {
        self.db
            .search_user_data(
                UserDataNamespace::Internal(
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                ),
                None,
            )
            .await
    }

    /// Returns all web page resources tracker job references that have jobs that need are pending.
    pub fn get_pending_resources_tracker_jobs(
        &self,
    ) -> impl Stream<Item = anyhow::Result<UserData<Uuid>>> + '_ {
        try_stream! {
            let jobs = self.db.get_stopped_scheduler_jobs_by_extra(
                MAX_JOBS_PAGE_SIZE,
                &[SchedulerJob::ResourcesTrackersTrigger as u8],
            );
            pin_mut!(jobs);

            while let Some(job_data) = jobs.next().await {
                let job_id = job_data?
                    .id
                    .ok_or_else(|| anyhow!("Job without ID"))?
                    .into();
                if let Some(job) = self.get_resources_tracker_job_by_id(job_id).await? {
                    yield job;
                } else {
                    log::error!("Found job without corresponding web page resources tracker: {}", job_id);
                }
            }
        }
    }

    /// Returns all web page resources trackers that have jobs that need to be scheduled.
    pub async fn get_resources_tracker_job_by_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<UserData<Uuid>>> {
        let mut jobs = self
            .db
            .search_user_data(
                UserDataNamespace::Internal(
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                ),
                job_id,
            )
            .await?;
        if jobs.len() > 1 {
            bail!("Found more than one job with the same ID: {}", job_id);
        }

        if jobs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(jobs.remove(0)))
        }
    }

    /// Upserts web page resources tracker job.
    pub async fn upsert_resources_tracker_job(
        &self,
        user_id: UserId,
        tracker_name: &str,
        job_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        self.db
            .upsert_user_data(
                (
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                    tracker_name,
                ),
                UserData::new_with_key(user_id, tracker_name, job_id, OffsetDateTime::now_utc()),
            )
            .await
    }

    /// Removes web page resources tracker job.
    pub async fn remove_resources_tracker_job(
        &self,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<()> {
        self.db
            .remove_user_data(
                user_id,
                (
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                    tracker_name,
                ),
            )
            .await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with web scraping data.
    pub fn web_scraping(&self) -> WebScrapingApi<&Config, DR, ET> {
        WebScrapingApi::new(&self.config, &self.db, &self.network)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        tests::{
            mock_config, mock_db, mock_network, mock_user, MockWebPageResourcesTrackerBuilder,
        },
        users::{PublicUserDataNamespace, User},
        utils::{
            web_scraping::WebScrapingApi, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesRevision, WebPageResourcesTracker,
            WebPageResourcesTrackerScripts,
        },
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    async fn initialize_mock_db(user: &User) -> anyhow::Result<Database> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn properly_saves_new_web_page_resource_trackers() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(tracker_one.name.clone(), tracker_one.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (tracker_one.name.clone(), tracker_one.clone()),
                (tracker_two.name.clone(), tracker_two.clone())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs.len(), 1);
        assert_eq!(tracker_jobs[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs[0].key, Some(tracker_two.name));
        assert!(tracker_jobs[0].value.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_updates_existing_web_page_resource_trackers() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(tracker_one.name.clone(), tracker_one)]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            2,
        )?
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter: Some("return resource.url !== undefined;".to_string()),
        })
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(tracker_one.name.clone(), tracker_one)]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_web_page_resource_trackers() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();

        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (tracker_one.name.clone(), tracker_one.clone()),
                (tracker_two.name.clone(), tracker_two.clone())
            ]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        api.remove_resources_tracker(mock_user.id, &tracker_one.name)
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(tracker_two.name.clone(), tracker_two.clone())]
                .into_iter()
                .collect::<HashMap<_, _>>()
        );

        api.remove_resources_tracker(mock_user.id, &tracker_two.name)
            .await?;

        let user_data = mock_db
            .get_user_data::<HashMap<String, WebPageResourcesTracker>>(
                mock_user.id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?;
        assert!(user_data.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_saves_web_page_resources() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one.clone()]);
        assert!(tracker_two_resources.is_empty());

        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(
            tracker_one_resources,
            vec![resources_one.clone(), resources_two.clone()]
        );
        assert!(tracker_two_resources.is_empty());

        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three]);

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_replaces_web_page_resources_with_the_same_timestamp() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert!(tracker_one_resources.is_empty());

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(tracker_one_resources, vec![resources_one.clone()]);

        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(
            tracker_one_resources,
            vec![resources_one.clone(), resources_two.clone()]
        );

        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=3")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_three]);

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_replaces_web_page_resources_with_no_diff() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert!(tracker_one_resources.is_empty());

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(tracker_one_resources, vec![resources_one.clone()]);

        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(
            tracker_one_resources,
            vec![resources_one.clone(), resources_two.clone()]
        );

        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946730900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_three]);

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_web_page_resources() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720700)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        api.remove_tracked_resources(mock_user.id, &tracker_one)
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three]);

        api.remove_tracked_resources(mock_user.id, &tracker_two)
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_web_page_resources_when_tracker_is_removed() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720700)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        api.remove_resources_tracker(mock_user.id, &tracker_one.name)
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three]);

        api.remove_resources_tracker(mock_user.id, &tracker_two.name)
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_web_page_resources_when_tracker_url_changed() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720700)?,
            scripts: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
            styles: vec![],
        };
        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        let resources_three = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720900)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(
            tracker_one_resources,
            vec![resources_one.clone(), resources_two.clone()]
        );
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update tracker without changing URL.
        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            4,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update tracker with changing URL.
        let tracker_one = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1235/my/app?q=2",
            4,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update second tracker with changing URL.
        let tracker_two = MockWebPageResourcesTrackerBuilder::create(
            "name_two",
            "http://localhost:1235/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_updates_job_when_tracker_schedule_changed() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker without adding schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            4,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker with schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            4,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_1 = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_1.len(), 1);
        assert_eq!(tracker_jobs_rev_1[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs_rev_1[0].key, Some(tracker.name));
        assert!(tracker_jobs_rev_1[0].value.is_none());

        // Update tracker without updating schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            5,
        )?
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter: Some("script".to_string()),
        })
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_2 = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_1, tracker_jobs_rev_2);

        // Update tracker job.
        api.upsert_resources_tracker_job(
            mock_user.id,
            &tracker.name,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;
        let tracker_jobs_rev_3 = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs_rev_3.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        // Update tracker with a new schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            5,
        )?
        .with_schedule("0 1 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_4 = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_4.len(), 1);
        assert_eq!(tracker_jobs_rev_4[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs_rev_4[0].key, Some(tracker.name));
        assert!(tracker_jobs_rev_4[0].value.is_none());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        // Remove schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            5,
        )?
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_5 = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs_rev_5.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_job_when_tracker_revisions_disabled() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].user_id, mock_user.id);
        assert_eq!(jobs[0].key, Some(tracker.name));
        assert!(jobs[0].value.is_none());

        // Disable revisions.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            0,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(jobs.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_job_when_tracker_is_removed() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        // Update tracker with schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            4,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs.len(), 1);
        assert_eq!(tracker_jobs[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs[0].key, Some(tracker.name.clone()));
        assert!(tracker_jobs[0].value.is_none());

        api.remove_resources_tracker(mock_user.id, &tracker.name)
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker with schedule.
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "name_one",
            "http://localhost:1234/my/app?q=2",
            4,
        )?
        .with_schedule("0 0 * * *")
        .build();
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        api.upsert_resources_tracker_job(
            mock_user.id,
            &tracker.name,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;
        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        api.remove_resources_tracker(mock_user.id, &tracker.name)
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_manipulate_tracker_jobs() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mock_db = initialize_mock_db(&mock_user).await?;
        let mock_network = mock_network();
        let api = WebScrapingApi::new(mock_config()?, &mock_db, &mock_network);

        let tracker_name = "name_one".to_string();

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        api.upsert_resources_tracker_job(mock_user.id, &tracker_name, None)
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs.len(), 1);
        assert_eq!(tracker_jobs[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs[0].key, Some(tracker_name.clone()));
        assert!(tracker_jobs[0].value.is_none());

        api.upsert_resources_tracker_job(
            mock_user.id,
            &tracker_name,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        api.remove_resources_tracker_job(mock_user.id, &tracker_name)
            .await?;

        let tracker_jobs = api.get_unscheduled_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        Ok(())
    }
}
