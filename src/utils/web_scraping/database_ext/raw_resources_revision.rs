use crate::utils::{WebPageResource, WebPageResourceContent, WebPageResourcesRevision};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResourcesRevision {
    pub id: Vec<u8>,
    pub tracker_id: Vec<u8>,
    pub value: Vec<u8>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub(super) struct RawResourcesRevisionValue {
    pub scripts: Vec<(Option<Url>, Option<WebPageResourceContent>)>,
    pub styles: Vec<(Option<Url>, Option<WebPageResourceContent>)>,
}

impl TryFrom<RawResourcesRevision> for WebPageResourcesRevision {
    type Error = anyhow::Error;

    fn try_from(raw: RawResourcesRevision) -> Result<Self, Self::Error> {
        let raw_value = postcard::from_bytes::<RawResourcesRevisionValue>(&raw.value)?;
        let convert_to_resource = |resources: Vec<(
            Option<Url>,
            Option<WebPageResourceContent>,
        )>|
         -> Vec<WebPageResource> {
            resources
                .into_iter()
                .map(|(url, content)| WebPageResource {
                    url,
                    content,
                    diff_status: None,
                })
                .collect()
        };
        Ok(Self {
            id: Uuid::from_slice(raw.id.as_slice())?,
            tracker_id: Uuid::from_slice(raw.tracker_id.as_slice())?,
            scripts: convert_to_resource(raw_value.scripts),
            styles: convert_to_resource(raw_value.styles),
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl TryFrom<&WebPageResourcesRevision> for RawResourcesRevision {
    type Error = anyhow::Error;

    fn try_from(item: &WebPageResourcesRevision) -> Result<Self, Self::Error> {
        let raw_value = RawResourcesRevisionValue {
            scripts: item
                .scripts
                .iter()
                .map(|resource| (resource.url.clone(), resource.content.clone()))
                .collect(),
            styles: item
                .styles
                .iter()
                .map(|resource| (resource.url.clone(), resource.content.clone()))
                .collect(),
        };

        Ok(Self {
            id: item.id.as_ref().to_vec(),
            tracker_id: item.tracker_id.as_ref().to_vec(),
            value: postcard::to_stdvec(&raw_value)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawResourcesRevision;
    use crate::utils::{
        WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourcesRevision,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_resource_revision() -> anyhow::Result<()> {
        assert_eq!(
            WebPageResourcesRevision::try_from(RawResourcesRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                value: vec![
                    1, 0, 1, 2, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 217, 2,
                    2, 0, 0, 1, 30, 104, 116, 116, 112, 115, 58, 47, 47, 115, 101, 99, 117, 116,
                    105, 108, 115, 46, 100, 101, 118, 47, 115, 99, 114, 105, 112, 116, 46, 106,
                    115, 1, 1, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 123
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageResourcesRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                styles: vec![
                    WebPageResource {
                        url: None,
                        content: None,
                        diff_status: None,
                    },
                    WebPageResource {
                        url: Some("https://secutils.dev/script.js".parse()?),
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("some-digest".to_string(),),
                            size: 123,
                        }),
                        diff_status: None,
                    }
                ],
                scripts: vec![WebPageResource {
                    url: None,
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Tlsh("some-digest".to_string(),),
                        size: 345,
                    }),
                    diff_status: None,
                }],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_resource_revision() -> anyhow::Result<()> {
        assert_eq!(
            RawResourcesRevision::try_from(&WebPageResourcesRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                styles: vec![
                    WebPageResource {
                        url: None,
                        content: None,
                        diff_status: None,
                    },
                    WebPageResource {
                        url: Some("https://secutils.dev/script.js".parse()?),
                        content: Some(WebPageResourceContent {
                            data: WebPageResourceContentData::Sha1("some-digest".to_string(),),
                            size: 123,
                        }),
                        diff_status: None,
                    }
                ],
                scripts: vec![WebPageResource {
                    url: None,
                    content: Some(WebPageResourceContent {
                        data: WebPageResourceContentData::Tlsh("some-digest".to_string(),),
                        size: 345,
                    }),
                    diff_status: None,
                }],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawResourcesRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                value: vec![
                    1, 0, 1, 2, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 217, 2,
                    2, 0, 0, 1, 30, 104, 116, 116, 112, 115, 58, 47, 47, 115, 101, 99, 117, 116,
                    105, 108, 115, 46, 100, 101, 118, 47, 115, 99, 114, 105, 112, 116, 46, 106,
                    115, 1, 1, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 123
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
