use crate::{users::UserId, utils::WebPageResourcesTrackerSettings};
use serde::Serialize;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResourcesTracker {
    /// Unique web page resources tracker id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the web page resources tracker.
    pub name: String,
    /// URL of the web page to track resources for.
    pub url: Url,
    /// Id of the user who owns the tracker.
    #[serde(skip_serializing)]
    pub user_id: UserId,
    /// ID of the optional job that triggers resource checking. If `None` when `schedule` is set,
    /// then the job is not scheduled it.
    #[serde(skip_serializing)]
    pub job_id: Option<Uuid>,
    /// Settings of the web page resources tracker.
    pub settings: WebPageResourcesTrackerSettings,
    /// Date and time when the web page resources tracker was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockWebPageResourcesTrackerBuilder,
        utils::web_scraping::resources::WebPageResourcesTrackerScripts,
    };
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let tracker = MockWebPageResourcesTrackerBuilder::create(
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

        let tracker = MockWebPageResourcesTrackerBuilder::create(
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

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource;".to_string()),
        })
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

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts::default())
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

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_delay_millis(2500)
        .with_schedule("0 0 * * *")
        .with_scripts(WebPageResourcesTrackerScripts::default())
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
            "enableNotifications": false
          },
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }
}
