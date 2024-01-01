use crate::utils::web_scraping::WebPageTrackerTag;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageDataRevision<Tag: WebPageTrackerTag> {
    /// Unique web page tracker data revision id (UUIDv7).
    pub id: Uuid,
    /// Id of the tracker captured data belongs to.
    #[serde(skip_serializing)]
    pub tracker_id: Uuid,
    /// Web page data revision value.
    pub data: Tag::TrackerData,
    /// Timestamp indicating when data was fetched.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::{
        WebPageDataRevision, WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourcesData, WebPageResourcesTrackerTag,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebPageDataRevision::<WebPageResourcesTrackerTag> {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            created_at: OffsetDateTime::from_unix_timestamp(
                946720800,
            )?,
            data: WebPageResourcesData {
                scripts: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app?q=2")?),
                    content: Some(WebPageResourceContent{
                        data: WebPageResourceContentData::Sha1("some-digest".to_string()),
                        size: 123
                    }),
                    diff_status: None,
                }],
                styles: vec![WebPageResource {
                    url: Some(Url::parse("http://localhost:1234/my/app.css?q=2")?),
                    content: Some(WebPageResourceContent{
                        data: WebPageResourceContentData::Sha1("another-digest".to_string()),
                        size: 321
                    }),
                    diff_status: None,
                }]
            }
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "data": {
            "scripts": [
              {
                "url": "http://localhost:1234/my/app?q=2",
                "content": {
                  "data": {
                    "sha1": "some-digest"
                  },
                  "size": 123
                }
              }
            ],
            "styles": [
              {
                "url": "http://localhost:1234/my/app.css?q=2",
                "content": {
                  "data": {
                    "sha1": "another-digest"
                  },
                  "size": 321
                }
              }
            ]
          },
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }
}
