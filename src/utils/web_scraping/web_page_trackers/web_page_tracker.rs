use crate::{
    users::UserId,
    utils::{WebPageTrackerSettings, WebPageTrackerTag},
};
use serde::Serialize;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageTracker<Tag: WebPageTrackerTag> {
    /// Unique web page tracker id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the web page tracker.
    pub name: String,
    /// URL of the web page to track.
    pub url: Url,
    /// Id of the user who owns the tracker.
    #[serde(skip_serializing)]
    pub user_id: UserId,
    /// ID of the optional job that triggers web page checking. If `None` when `schedule` is set,
    /// then the job is not scheduled it.
    #[serde(skip_serializing)]
    pub job_id: Option<Uuid>,
    /// Settings of the web page tracker.
    pub settings: WebPageTrackerSettings,
    /// Optional meta data of the web page tracker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Tag::TrackerMeta>,
    /// Date and time when the web page tracker was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockWebPageTrackerBuilder,
        utils::{WebPageResourcesTrackerTag, WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME},
    };
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "settings": {
            "revisions": 3,
            "delay": 2500,
            "enableNotifications": true
          },
          "createdAt": 946720800
        }
        "###);

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "settings": {
            "revisions": 3,
            "schedule": "0 0 * * *",
            "delay": 2500,
            "enableNotifications": true
          },
          "createdAt": 946720800
        }
        "###);

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(
            [(
                WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                "return resource;".to_string(),
            )]
            .into_iter()
            .collect(),
        )
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "settings": {
            "revisions": 3,
            "schedule": "0 0 * * *",
            "delay": 2500,
            "scripts": {
              "resourceFilterMap": "return resource;"
            },
            "enableNotifications": true
          },
          "createdAt": 946720800
        }
        "###);

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(Default::default())
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "settings": {
            "revisions": 3,
            "schedule": "0 0 * * *",
            "delay": 2500,
            "scripts": {},
            "enableNotifications": true
          },
          "createdAt": 946720800
        }
        "###);

        let tracker = MockWebPageTrackerBuilder::<WebPageResourcesTrackerTag>::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(Default::default())
        .without_notifications()
        .build();
        assert_json_snapshot!(tracker, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "url": "http://localhost:1234/my/app?q=2",
          "settings": {
            "revisions": 3,
            "schedule": "0 0 * * *",
            "delay": 2500,
            "scripts": {},
            "enableNotifications": false
          },
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }
}
