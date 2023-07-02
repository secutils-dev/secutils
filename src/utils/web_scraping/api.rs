use crate::{
    api::{Api, DictionaryDataUserDataSetter, UserDataSetter},
    datastore::PrimaryDb,
    users::{PublicUserDataNamespace, UserData, UserId},
    utils::{WebPageResourcesRevision, WebPageResourcesTracker},
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};
use time::OffsetDateTime;

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
    pub async fn save_web_page_resources_tracker(
        &self,
        user_id: UserId,
        tracker: WebPageResourcesTracker,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        // First retrieve tracker and check if it has different URL. If so, we need to remove all
        // tracked resources.
        let existing_tracker = self
            .get_web_page_resources_tracker(user_id, &tracker.name)
            .await?;
        let user_data_setter = UserDataSetter::new(user_id, &self.primary_db);
        if let Some(existing_tracker) = existing_tracker {
            if existing_tracker.url != tracker.url {
                log::debug!(
                    "Web resources tracker \"{}\" (user ID: {:?}) changed URL, clearing web resources history.",
                    existing_tracker.name,
                    user_id
                );
                user_data_setter
                    .remove((
                        PublicUserDataNamespace::WebPageResourcesTrackers,
                        existing_tracker.name.as_str(),
                    ))
                    .await?;
            }
        }

        DictionaryDataUserDataSetter::upsert(
            &user_data_setter,
            PublicUserDataNamespace::WebPageResourcesTrackers,
            UserData::new(
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
    pub async fn remove_web_page_resources_tracker(
        &self,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<()> {
        // First delete tracked resources.
        let user_data_setter = UserDataSetter::new(user_id, &self.primary_db);
        user_data_setter
            .remove((
                PublicUserDataNamespace::WebPageResourcesTrackers,
                tracker_name,
            ))
            .await?;

        // Then delete the tracker itself.
        DictionaryDataUserDataSetter::upsert(
            &user_data_setter,
            PublicUserDataNamespace::WebPageResourcesTrackers,
            UserData::new(
                [(tracker_name.to_string(), None)]
                    .into_iter()
                    .collect::<BTreeMap<_, Option<WebPageResourcesTracker>>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await
    }

    /// Returns web page resources tracker by its name.
    pub async fn get_web_page_resources_tracker(
        &self,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<Option<WebPageResourcesTracker>> {
        Ok(UserDataSetter::new(user_id, &self.primary_db)
            .get::<BTreeMap<String, WebPageResourcesTracker>>(
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .and_then(|mut map| map.value.remove(tracker_name)))
    }

    /// Returns all stored webpage resources for the specified web page resources tracker.
    pub async fn get_web_page_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<Vec<WebPageResourcesRevision>> {
        Ok(UserDataSetter::new(user_id, &self.primary_db)
            .get::<Vec<WebPageResourcesRevision>>((
                PublicUserDataNamespace::WebPageResourcesTrackers,
                tracker.name.as_str(),
            ))
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default())
    }

    /// Persists resources for the specified web page resources tracker.
    pub async fn save_web_page_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
        revision: WebPageResourcesRevision,
    ) -> anyhow::Result<()> {
        let user_data_setter = UserDataSetter::new(user_id, &self.primary_db);
        let user_data_key = (
            PublicUserDataNamespace::WebPageResourcesTrackers,
            tracker.name.as_str(),
        );

        // Enforce revisions limit and displace the oldest one.
        let mut revisions = user_data_setter
            .get::<VecDeque<WebPageResourcesRevision>>(user_data_key)
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default();
        if revisions.len() == tracker.revisions {
            revisions.pop_front();
        }
        revisions.push_back(revision);

        user_data_setter
            .upsert(
                user_data_key,
                UserData::new(revisions, OffsetDateTime::now_utc()),
            )
            .await
    }

    /// Removes all persisted resources for the specified web page resources tracker.
    pub async fn remove_tracked_web_page_resources(
        &self,
        user_id: UserId,
        tracker: &WebPageResourcesTracker,
    ) -> anyhow::Result<()> {
        UserDataSetter::new(user_id, &self.primary_db)
            .remove((
                PublicUserDataNamespace::WebPageResourcesTrackers,
                tracker.name.as_str(),
            ))
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
        api::UserDataSetter,
        datastore::PrimaryDb,
        tests::{mock_db, mock_user},
        users::{PublicUserDataNamespace, User},
        utils::{
            web_scraping::WebScrapingApi, WebPageResource, WebPageResourceContent,
            WebPageResourcesRevision, WebPageResourcesTracker,
        },
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;
    use url::Url;

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
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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

        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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

        api.remove_web_page_resources_tracker(mock_user.id, &tracker_one.name)
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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

        api.remove_web_page_resources_tracker(mock_user.id, &tracker_two.name)
            .await?;

        let user_data = UserDataSetter::new(mock_user.id, &mock_db)
            .get::<HashMap<String, WebPageResourcesTracker>>(
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
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert_eq!(tracker_one_resources, vec![resources_one.clone()]);
        assert!(tracker_two_resources.is_empty());

        let resources_two = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                content: None,
                diff_status: None,
            }],
        };
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
                    digest: "some-digest".to_string(),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_web_page_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three]);

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
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    digest: "some-digest".to_string(),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        api.remove_tracked_web_page_resources(mock_user.id, &tracker_one)
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three]);

        api.remove_tracked_web_page_resources(mock_user.id, &tracker_two)
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
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
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    digest: "some-digest".to_string(),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        api.remove_web_page_resources_tracker(mock_user.id, &tracker_one.name)
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three]);

        api.remove_web_page_resources_tracker(mock_user.id, &tracker_two.name)
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
        };
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1234/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let resources_one = WebPageResourcesRevision {
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
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
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            scripts: vec![],
            styles: vec![WebPageResource {
                url: Some(Url::parse("http://localhost:4321/my/app?q=2")?),
                content: Some(WebPageResourceContent {
                    digest: "some-digest".to_string(),
                    size: 345,
                }),
                diff_status: None,
            }],
        };
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_one.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_one, resources_two.clone())
            .await?;
        api.save_web_page_resources(mock_user.id, &tracker_two, resources_three.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
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
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert_eq!(tracker_one_resources, vec![resources_one, resources_two]);
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update tracker with changing URL.
        let tracker_one = WebPageResourcesTracker {
            name: "name_one".to_string(),
            url: Url::parse("http://localhost:1235/my/app?q=2")?,
            revisions: 4,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_one.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert_eq!(tracker_two_resources, vec![resources_three.clone()]);

        // Update second tracker with changing URL.
        let tracker_two = WebPageResourcesTracker {
            name: "name_two".to_string(),
            url: Url::parse("http://localhost:1235/my/app?q=2")?,
            revisions: 3,
        };
        api.save_web_page_resources_tracker(mock_user.id, tracker_two.clone())
            .await?;

        let tracker_one_resources = api
            .get_web_page_resources(mock_user.id, &tracker_one)
            .await?;
        let tracker_two_resources = api
            .get_web_page_resources(mock_user.id, &tracker_two)
            .await?;
        assert!(tracker_one_resources.is_empty());
        assert!(tracker_two_resources.is_empty());

        Ok(())
    }
}
