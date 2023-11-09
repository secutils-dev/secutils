use crate::utils::{WebPageDataRevision, WebPageTrackerTag};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawWebPageDataRevision {
    pub id: Vec<u8>,
    pub tracker_id: Vec<u8>,
    pub data: Vec<u8>,
    pub created_at: i64,
}

impl<Tag: WebPageTrackerTag> TryFrom<RawWebPageDataRevision> for WebPageDataRevision<Tag> {
    type Error = anyhow::Error;

    fn try_from(raw: RawWebPageDataRevision) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::from_slice(raw.id.as_slice())?,
            tracker_id: Uuid::from_slice(raw.tracker_id.as_slice())?,
            data: postcard::from_bytes::<Tag::TrackerData>(&raw.data)?,
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl<Tag: WebPageTrackerTag> TryFrom<&WebPageDataRevision<Tag>> for RawWebPageDataRevision {
    type Error = anyhow::Error;

    fn try_from(item: &WebPageDataRevision<Tag>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: item.id.as_ref().to_vec(),
            tracker_id: item.tracker_id.as_ref().to_vec(),
            data: postcard::to_stdvec(&item.data)?,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawWebPageDataRevision;
    use crate::utils::{
        WebPageDataRevision, WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourcesData, WebPageResourcesTrackerTag,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_web_page_data_revision() -> anyhow::Result<()> {
        assert_eq!(
            WebPageDataRevision::<WebPageResourcesTrackerTag>::try_from(RawWebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![
                    1, 0, 1, 2, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 217, 2,
                    2, 0, 0, 1, 30, 104, 116, 116, 112, 115, 58, 47, 47, 115, 101, 99, 117, 116,
                    105, 108, 115, 46, 100, 101, 118, 47, 115, 99, 114, 105, 112, 116, 46, 106,
                    115, 1, 1, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 123
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            WebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: WebPageResourcesData {
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
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_web_page_data_revision() -> anyhow::Result<()> {
        assert_eq!(
            RawWebPageDataRevision::try_from(&WebPageDataRevision::<WebPageResourcesTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: WebPageResourcesData {
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
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawWebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002")
                    .as_bytes()
                    .to_vec(),
                data: vec![
                    1, 1, 2, 11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 217, 2, 2,
                    1, 30, 104, 116, 116, 112, 115, 58, 47, 47, 115, 101, 99, 117, 116, 105, 108,
                    115, 46, 100, 101, 118, 47, 115, 99, 114, 105, 112, 116, 46, 106, 115, 1, 1,
                    11, 115, 111, 109, 101, 45, 100, 105, 103, 101, 115, 116, 123
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
