use crate::utils::web_scraping::{WebPageContentTrackerTag, WebPageDataRevision};
use handlebars::JsonRender;
use serde_json::Value as JSONValue;
use similar::TextDiff;

/// Pretty prints the web page content revision data.
fn web_page_content_revision_pretty_print(data: &str) -> anyhow::Result<String> {
    let json_data = serde_json::from_str::<JSONValue>(data)?;
    Ok(
        if json_data.is_object() || json_data.is_array() || json_data.is_null() {
            serde_json::to_string_pretty(&json_data)?
        } else {
            json_data.render()
        },
    )
}

/// Takes multiple web page content revisions and calculates the diff.
pub fn web_page_content_revisions_diff(
    revisions: Vec<WebPageDataRevision<WebPageContentTrackerTag>>,
) -> anyhow::Result<Vec<WebPageDataRevision<WebPageContentTrackerTag>>> {
    if revisions.len() < 2 {
        return Ok(revisions);
    }

    let mut revisions_diff = Vec::with_capacity(revisions.len());
    let mut peekable_revisions = revisions.into_iter().rev().peekable();
    while let Some(current_revision) = peekable_revisions.next() {
        if let Some(previous_revision) = peekable_revisions.peek() {
            let current_value = web_page_content_revision_pretty_print(&current_revision.data)?;
            let previous_value = web_page_content_revision_pretty_print(&previous_revision.data)?;

            revisions_diff.push(WebPageDataRevision {
                data: TextDiff::from_lines(&previous_value, &current_value)
                    .unified_diff()
                    .context_radius(10000)
                    .missing_newline_hint(false)
                    .to_string(),
                ..current_revision
            });
        } else {
            revisions_diff.push(current_revision);
        }
    }

    Ok(revisions_diff.into_iter().rev().collect())
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::{
        web_page_content_revisions_diff, WebPageContentTrackerTag, WebPageDataRevision,
    };
    use insta::assert_debug_snapshot;
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn correctly_calculates_web_page_content_diff() -> anyhow::Result<()> {
        let revisions = vec![
            WebPageDataRevision::<WebPageContentTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: "\"Hello World\"".to_string(),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebPageDataRevision::<WebPageContentTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: "\"Hello New World\"".to_string(),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
        ];

        let diff = web_page_content_revisions_diff(revisions)?;
        assert_debug_snapshot!(diff, @r###"
        [
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000001,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "\"Hello World\"",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000002,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "@@ -1 +1 @@\n-Hello World\n+Hello New World\n",
                created_at: 2000-01-01 10:00:01.0 +00:00:00,
            },
        ]
        "###);

        Ok(())
    }

    #[test]
    fn correctly_calculates_web_page_content_diff_for_json() -> anyhow::Result<()> {
        let revisions = vec![WebPageDataRevision::<WebPageContentTrackerTag> {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
            data: json!({ "property": "one", "secondProperty": "two" }).to_string(),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        }];

        let diff = web_page_content_revisions_diff(revisions)?;
        assert_debug_snapshot!(diff, @r###"
        [
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000001,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "{\"property\":\"one\",\"secondProperty\":\"two\"}",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        ]
        "###);

        let revisions = vec![
            WebPageDataRevision::<WebPageContentTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: json!({ "property": "one", "secondProperty": "two" }).to_string(),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebPageDataRevision::<WebPageContentTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000002"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data: json!({ "property": "one" }).to_string(),
                created_at: OffsetDateTime::from_unix_timestamp(946720801)?,
            },
            WebPageDataRevision::<WebPageContentTrackerTag> {
                id: uuid!("00000000-0000-0000-0000-000000000003"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                data:
                    json!({ "property": "one", "secondProperty": "two", "thirdProperty": "three" })
                        .to_string(),
                created_at: OffsetDateTime::from_unix_timestamp(946720802)?,
            },
        ];

        let diff = web_page_content_revisions_diff(revisions)?;
        assert_debug_snapshot!(diff, @r###"
        [
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000001,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "{\"property\":\"one\",\"secondProperty\":\"two\"}",
                created_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000002,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "@@ -1,4 +1,3 @@\n {\n-  \"property\": \"one\",\n-  \"secondProperty\": \"two\"\n+  \"property\": \"one\"\n }\n",
                created_at: 2000-01-01 10:00:01.0 +00:00:00,
            },
            WebPageDataRevision {
                id: 00000000-0000-0000-0000-000000000003,
                tracker_id: 00000000-0000-0000-0000-000000000002,
                data: "@@ -1,3 +1,5 @@\n {\n-  \"property\": \"one\"\n+  \"property\": \"one\",\n+  \"secondProperty\": \"two\",\n+  \"thirdProperty\": \"three\"\n }\n",
                created_at: 2000-01-01 10:00:02.0 +00:00:00,
            },
        ]
        "###);

        Ok(())
    }
}
