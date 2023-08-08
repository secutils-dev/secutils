use crate::{
    api::{Api, DictionaryDataUserDataSetter},
    datastore::PrimaryDb,
    users::{
        InternalUserDataNamespace, PublicUserDataNamespace, UserData, UserDataKey,
        UserDataNamespace, UserId,
    },
    utils::{WebPageResourcesRevision, WebPageResourcesTracker},
};
use anyhow::bail;
use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};
use time::OffsetDateTime;
use uuid::Uuid;

pub struct WebScrapingApi<'a> {
    primary_db: Cow<'a, PrimaryDb>,
}

impl<'a> WebScrapingApi<'a> {
    /// Creates WebScraping API.
    pub fn new(primary_db: &'a PrimaryDb) -> Self {
        Self {
            primary_db: Cow::Borrowed(primary_db),
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
        let mut job_schedule_changed = false;
        if let Some(existing_tracker) = &existing_tracker {
            if existing_tracker.url != tracker.url {
                log::debug!(
                    "Web resources tracker \"{}\" (user ID: {:?}) changed URL, clearing web resources history.",
                    existing_tracker.name,
                    user_id
                );
                self.primary_db
                    .remove_user_data(
                        user_id,
                        (
                            PublicUserDataNamespace::WebPageResourcesTrackers,
                            existing_tracker.name.as_str(),
                        ),
                    )
                    .await?;
            }

            if existing_tracker.schedule != tracker.schedule {
                job_schedule_changed = true;
            }
        }

        // If schedule has changed, or it's a new tracker, we need to either reset or remove scheduled job.
        match tracker.schedule {
            Some(_) if job_schedule_changed || existing_tracker.is_none() => {
                self.upsert_resources_tracker_job(user_id, &tracker.name, None)
                    .await?;
            }
            None if job_schedule_changed => {
                self.remove_resources_tracker_job(user_id, &tracker.name)
                    .await?;
            }
            _ => {}
        }

        DictionaryDataUserDataSetter::upsert(
            &self.primary_db,
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
        self.primary_db
            .remove_user_data(
                user_id,
                (
                    PublicUserDataNamespace::WebPageResourcesTrackers,
                    tracker_name,
                ),
            )
            .await?;
        self.primary_db
            .remove_user_data(
                user_id,
                (
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                    tracker_name,
                ),
            )
            .await?;

        // Then delete the tracker itself.
        DictionaryDataUserDataSetter::upsert(
            &self.primary_db,
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
            .primary_db
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
            .primary_db
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
        revision: WebPageResourcesRevision,
    ) -> anyhow::Result<()> {
        let user_data_key = UserDataKey::from((
            PublicUserDataNamespace::WebPageResourcesTrackers,
            tracker.name.as_str(),
        ));

        // Enforce revisions limit and displace the oldest one.
        let mut revisions = self
            .primary_db
            .get_user_data::<VecDeque<WebPageResourcesRevision>>(user_id, user_data_key)
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default();

        // Check if there is a revision with the same timestamp. If so, we need to replace it.
        if let Some(position) = revisions
            .iter()
            .position(|r| r.timestamp == revision.timestamp)
        {
            revisions[position] = revision;
        } else {
            if revisions.len() == tracker.revisions {
                revisions.pop_front();
            }
            revisions.push_back(revision);
        }

        self.primary_db
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

    /// Removes all persisted resources for the specified web page resources tracker.
    pub async fn remove_tracked_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        self.primary_db
            .remove_user_data(
                user_id,
                (
                    PublicUserDataNamespace::WebPageResourcesTrackers,
                    tracker.name.as_str(),
                ),
            )
            .await
    }

    /// Returns all web page resources trackers that have jobs that need to be scheduled.
    pub async fn get_all_pending_resources_tracker_jobs(
        &self,
    ) -> anyhow::Result<Vec<UserData<Option<Uuid>>>> {
        self.primary_db
            .search_user_data(
                UserDataNamespace::Internal(
                    InternalUserDataNamespace::WebPageResourcesTrackersJobs,
                ),
                None,
            )
            .await
    }

    /// Returns all web page resources trackers that have jobs that need to be scheduled.
    pub async fn get_resources_tracker_job_by_id(
        &self,
        job_id: Uuid,
    ) -> anyhow::Result<Option<UserData<Uuid>>> {
        let mut jobs = self
            .primary_db
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
        self.primary_db
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
        self.primary_db
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

impl Api {
    /// Returns an API to work with web scraping data.
    pub fn web_scraping(&self) -> WebScrapingApi {
        WebScrapingApi::new(&self.datastore.primary_db)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::PrimaryDb,
        tests::{mock_db, mock_user},
        users::{PublicUserDataNamespace, User},
        utils::{
            web_scraping::WebScrapingApi, WebPageResource, WebPageResourceContent,
            WebPageResourceContentData, WebPageResourcesRevision, WebPageResourcesTracker,
        },
    };
    use std::{collections::HashMap, time::Duration};
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn properly_saves_new_web_page_resource_trackers() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
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

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs.len(), 1);
        assert_eq!(tracker_jobs[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs[0].key, Some(tracker_two.name));
        assert!(tracker_jobs[0].value.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_updates_existing_web_page_resource_trackers() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 2,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
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
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
    async fn properly_replaces_web_page_resources_with_the_same_revision() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
    async fn properly_removes_web_page_resources() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update tracker with changing URL.
        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1235/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        api.upsert_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api.get_resources(mock_user.id, &tracker_one).await?;
        let tracker_two_resources = api.get_resources(mock_user.id, &tracker_two).await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update second tracker with changing URL.
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1235/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
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
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker without adding schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker with schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_1 = api.get_all_pending_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_1.len(), 1);
        assert_eq!(tracker_jobs_rev_1[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs_rev_1[0].key, Some(tracker.name));
        assert!(tracker_jobs_rev_1[0].value.is_none());

        // Update tracker without updating schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 5,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_2 = api.get_all_pending_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_1, tracker_jobs_rev_2);

        // Update tracker job.
        api.upsert_resources_tracker_job(
            mock_user.id,
            &tracker.name,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;
        let tracker_jobs_rev_3 = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs_rev_3.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        // Update tracker with a new schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 5,
            delay: Duration::from_millis(2000),
            schedule: Some("0 1 * * *".to_string()),
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_4 = api.get_all_pending_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs_rev_4.len(), 1);
        assert_eq!(tracker_jobs_rev_4[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs_rev_4[0].key, Some(tracker.name));
        assert!(tracker_jobs_rev_4[0].value.is_none());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        // Remove schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 5,
            delay: Duration::from_millis(2000),
            schedule: None,
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs_rev_5 = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs_rev_5.is_empty());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_job_when_tracker_is_removed() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);

        // Update tracker with schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert_eq!(tracker_jobs.len(), 1);
        assert_eq!(tracker_jobs[0].user_id, mock_user.id);
        assert_eq!(tracker_jobs[0].key, Some(tracker.name.clone()));
        assert!(tracker_jobs[0].value.is_none());

        api.remove_resources_tracker(mock_user.id, &tracker.name)
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        // Update tracker with schedule.
        let tracker = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 4,
            delay: Duration::from_millis(2000),
            schedule: Some("0 0 * * *".to_string()),
        };
        api.upsert_resources_tracker(mock_user.id, tracker.clone())
            .await?;

        api.upsert_resources_tracker_job(
            mock_user.id,
            &tracker.name,
            Some(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")),
        )
        .await?;
        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        api.remove_resources_tracker(mock_user.id, &tracker.name)
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_manipulate_tracker_jobs() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = WebScrapingApi::new(&mock_db);
        let tracker_name = "name_one".to_string();

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        api.upsert_resources_tracker_job(mock_user.id, &tracker_name, None)
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
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

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_some());

        api.remove_resources_tracker_job(mock_user.id, &tracker_name)
            .await?;

        let tracker_jobs = api.get_all_pending_resources_tracker_jobs().await?;
        assert!(tracker_jobs.is_empty());

        let tracker_job = api
            .get_resources_tracker_job_by_id(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;
        assert!(tracker_job.is_none());

        Ok(())
    }
}
