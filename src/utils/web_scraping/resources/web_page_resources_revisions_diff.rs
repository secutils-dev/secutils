use crate::utils::{
    web_scraping::WebPageResourceDiffStatus, WebPageResource, WebPageResourcesRevision,
};
use itertools::{EitherOrBoth, Itertools};
use std::collections::HashMap;

/// Takes multiple web page resources revisions and updates diff status for resources in the
/// adjacent revisions.
pub fn web_page_resources_revisions_diff(
    revisions: Vec<WebPageResourcesRevision>,
) -> Vec<WebPageResourcesRevision> {
    // We can only calculate diff if there are at least two revisions.
    if revisions.len() < 2 {
        return revisions;
    }

    let mut revisions_diff = Vec::with_capacity(revisions.len());
    let mut peekable_revisions = revisions.into_iter().rev().peekable();
    while let Some(current_revision) = peekable_revisions.next() {
        if let Some(previous_revision) = peekable_revisions.peek() {
            revisions_diff.push(WebPageResourcesRevision {
                timestamp: current_revision.timestamp,
                scripts: web_page_resources_diff(
                    previous_revision.scripts.clone(),
                    current_revision.scripts,
                ),
                styles: web_page_resources_diff(
                    previous_revision.styles.clone(),
                    current_revision.styles,
                ),
            });
        } else {
            revisions_diff.push(current_revision);
        }
    }

    revisions_diff.into_iter().rev().collect()
}

/// Takes two sets of resources - current and previous revision - and returns a set of resources
/// with a populated diff status: added, removed, or changed.
fn web_page_resources_diff(
    resources_from: Vec<WebPageResource>,
    resources_to: Vec<WebPageResource>,
) -> Vec<WebPageResource> {
    // Most of the time resources don't change, so it makes sense to use original length as capacity.
    let mut resources_diff = Vec::with_capacity(resources_to.len());

    let mut resources_from_map = web_page_resources_to_map(resources_from);
    for (resource_key, resources_to) in web_page_resources_to_map(resources_to) {
        if let Some(resources_from) = resources_from_map.remove(resource_key.as_str()) {
            resources_diff.extend(
                resources_to
                    .into_iter()
                    .zip_longest(resources_from.into_iter())
                    .map(|resource_pair| match resource_pair {
                        EitherOrBoth::Both(resource_to, resource_from) => {
                            // NOTE: It's theoretically possible that there are multiple resources with the same
                            // URL and different content, but it's unlikely to happen in practice, so we don't
                            // handle this case and compare digest based on the position.
                            if resource_to.content != resource_from.content {
                                resource_to.with_diff_status(WebPageResourceDiffStatus::Changed)
                            } else {
                                resource_to
                            }
                        }
                        EitherOrBoth::Left(added_resource) => {
                            added_resource.with_diff_status(WebPageResourceDiffStatus::Added)
                        }
                        EitherOrBoth::Right(removed_resource) => {
                            removed_resource.with_diff_status(WebPageResourceDiffStatus::Removed)
                        }
                    }),
            )
        } else {
            // Add resources that were added, i.e. exist in the `resources_to` but not in the `resources_from`.
            resources_diff.extend(resources_to.into_iter().map(|added_resource| {
                added_resource.with_diff_status(WebPageResourceDiffStatus::Added)
            }));
        }
    }

    // Add resources that were removed, i.e. exist in the `resources_from` but not in the `resources_to`.
    resources_diff.extend(
        resources_from_map
            .into_values()
            .flatten()
            .map(|removed_resource| {
                removed_resource.with_diff_status(WebPageResourceDiffStatus::Removed)
            }),
    );

    resources_diff
}

/// Adds resources to a map where key is either URL (external resource) or digest (inline resource).
fn web_page_resources_to_map(
    resources: Vec<WebPageResource>,
) -> HashMap<String, Vec<WebPageResource>> {
    let mut resources_map = HashMap::new();
    for resource in resources {
        let resource_key = match (&resource.url, &resource.content) {
            (Some(url), _) => url.to_string(),
            (_, Some(content)) => content.digest.to_string(),
            _ => {
                log::warn!("Resource is missing both URL and content: {:?}", resource);
                continue;
            }
        };

        resources_map
            .entry(resource_key)
            .or_insert_with(Vec::new)
            .push(resource);
    }

    resources_map
}

#[cfg(test)]
mod tests {
    use super::web_page_resources_to_map;
    use crate::{
        tests::MockWebPageResourceBuilder,
        utils::{
            web_scraping::resources::{
                web_page_resources_revisions_diff,
                web_page_resources_revisions_diff::web_page_resources_diff,
            },
            WebPageResourcesRevision,
        },
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use url::Url;

    #[test]
    fn correctly_collects_resources_to_map() -> anyhow::Result<()> {
        let resource_one =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content("one-digest", 123)
                .build();
        let resource_two =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest", 321)
                .build();
        let resource_three = MockWebPageResourceBuilder::with_content("three-digest", 456).build();
        let resource_four =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest", 321)
                .build();
        let resource_five = MockWebPageResourceBuilder::with_content("four-digest", 456).build();
        let resource_six = MockWebPageResourceBuilder::with_content("three-digest", 456).build();

        let resources_map = web_page_resources_to_map(vec![
            resource_one.clone(),
            resource_two.clone(),
            resource_three.clone(),
            resource_four.clone(),
            resource_five.clone(),
            resource_six.clone(),
        ]);
        assert_eq!(resources_map.len(), 4);
        assert_eq!(
            resources_map.get("http://localhost/one"),
            Some(&vec![resource_one])
        );
        assert_eq!(
            resources_map.get("http://localhost/two"),
            Some(&vec![resource_two, resource_four])
        );
        assert_eq!(
            resources_map.get("three-digest"),
            Some(&vec![resource_three, resource_six])
        );
        assert_eq!(resources_map.get("four-digest"), Some(&vec![resource_five]));

        Ok(())
    }

    #[test]
    fn correctly_calculates_web_page_resources_diff() -> anyhow::Result<()> {
        // Not changed resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        let resource_one_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content("one-digest-no-change", 123)
                .build();
        let resource_one_inline_rev_1 =
            MockWebPageResourceBuilder::with_content("one-digest-inline-no-change", 123).build();
        let resource_one_multiple_one_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one-multiple")?)
                .set_content("one-digest-multiple-no-change", 123)
                .build();
        let resource_one_multiple_inline_rev_1 =
            MockWebPageResourceBuilder::with_content("one-digest-multiple-inline-no-change", 123)
                .build();

        // Changed resources:
        // 1. External, single resource
        // 2. External, multiple resources
        let resource_two_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest", 321)
                .build();
        let resource_two_multiple_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two-multiple")?)
                .set_content("two-digest-multiple", 321)
                .build();

        // Removed resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        let resource_three_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-removed")?)
                .set_content("three-digest-removed", 321)
                .build();
        let resource_three_inline_rev_1 =
            MockWebPageResourceBuilder::with_content("three-digest-inline-removed", 123).build();
        let resource_three_multiple_rev_1 = MockWebPageResourceBuilder::with_url(Url::parse(
            "http://localhost/three-multiple-removed",
        )?)
        .set_content("three-digest-multiple-removed", 321)
        .build();
        let resource_three_multiple_inline_rev_1 =
            MockWebPageResourceBuilder::with_content("three-digest-multiple-inline-removed", 123)
                .build();

        let resource_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content("one-digest-no-change", 123)
                .build();
        let resource_one_inline_rev_2 =
            MockWebPageResourceBuilder::with_content("one-digest-inline-no-change", 123).build();
        let resource_one_multiple_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one-multiple")?)
                .set_content("one-digest-multiple-no-change", 123)
                .build();
        let resource_one_multiple_inline_rev_2 =
            MockWebPageResourceBuilder::with_content("one-digest-multiple-inline-no-change", 123)
                .build();

        let resource_two_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest-changed", 321)
                .build();
        let resource_two_multiple_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two-multiple")?)
                .set_content("two-digest-multiple-changed", 321)
                .build();

        // Added resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        let resource_three_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content("three-digest-added", 321)
                .build();
        let resource_three_inline_rev_2 =
            MockWebPageResourceBuilder::with_content("three-digest-inline-added", 123).build();
        let resource_three_multiple_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-multiple")?)
                .set_content("three-digest-multiple-added", 321)
                .build();
        let resource_three_multiple_inline_rev_2 =
            MockWebPageResourceBuilder::with_content("three-digest-multiple-inline-added", 123)
                .build();

        let diff = web_page_resources_diff(
            vec![
                resource_one_rev_1,
                resource_one_inline_rev_1,
                resource_one_multiple_one_rev_1.clone(),
                resource_one_multiple_one_rev_1,
                resource_one_multiple_inline_rev_1.clone(),
                resource_one_multiple_inline_rev_1,
                resource_two_rev_1,
                resource_two_multiple_rev_1.clone(),
                resource_two_multiple_rev_1.clone(),
                resource_three_rev_1,
                resource_three_inline_rev_1,
                resource_three_multiple_rev_1.clone(),
                resource_three_multiple_rev_1,
                resource_three_multiple_inline_rev_1.clone(),
                resource_three_multiple_inline_rev_1,
            ],
            vec![
                resource_one_rev_2,
                resource_one_inline_rev_2,
                resource_one_multiple_one_rev_2.clone(),
                resource_one_multiple_one_rev_2,
                resource_one_multiple_inline_rev_2.clone(),
                resource_one_multiple_inline_rev_2,
                resource_two_rev_2,
                resource_two_multiple_rev_1,
                resource_two_multiple_rev_2,
                resource_three_rev_2,
                resource_three_inline_rev_2,
                resource_three_multiple_rev_2.clone(),
                resource_three_multiple_rev_2,
                resource_three_multiple_inline_rev_2.clone(),
                resource_three_multiple_inline_rev_2,
            ],
        );

        let diff_map = web_page_resources_to_map(diff);
        assert_eq!(diff_map.len(), 14);

        // 1. Check not changed resources (external)
        assert_json_snapshot!(diff_map.get("http://localhost/one"), @r###"
        [
          {
            "url": "http://localhost/one",
            "content": {
              "digest": "one-digest-no-change",
              "size": 123
            }
          }
        ]
        "###);

        // 2. Check not changed resources (internal)
        assert_json_snapshot!(diff_map.get("one-digest-inline-no-change"), @r###"
        [
          {
            "content": {
              "digest": "one-digest-inline-no-change",
              "size": 123
            }
          }
        ]
        "###);

        // 3. Check not changed resources (external, multiple)
        assert_json_snapshot!(diff_map.get("http://localhost/one-multiple"), @r###"
        [
          {
            "url": "http://localhost/one-multiple",
            "content": {
              "digest": "one-digest-multiple-no-change",
              "size": 123
            }
          },
          {
            "url": "http://localhost/one-multiple",
            "content": {
              "digest": "one-digest-multiple-no-change",
              "size": 123
            }
          }
        ]
        "###);

        // 4. Check not changed resources (internal, multiple)
        assert_json_snapshot!(diff_map.get("one-digest-multiple-inline-no-change"), @r###"
        [
          {
            "content": {
              "digest": "one-digest-multiple-inline-no-change",
              "size": 123
            }
          },
          {
            "content": {
              "digest": "one-digest-multiple-inline-no-change",
              "size": 123
            }
          }
        ]
        "###);

        // 5. Check changed resources (external)
        assert_json_snapshot!(diff_map.get("http://localhost/two"), @r###"
        [
          {
            "url": "http://localhost/two",
            "content": {
              "digest": "two-digest-changed",
              "size": 321
            },
            "diffStatus": "changed"
          }
        ]
        "###);

        // 6. Check changed resources (external, multiple)
        assert_json_snapshot!(diff_map.get("http://localhost/two-multiple"), @r###"
        [
          {
            "url": "http://localhost/two-multiple",
            "content": {
              "digest": "two-digest-multiple",
              "size": 321
            }
          },
          {
            "url": "http://localhost/two-multiple",
            "content": {
              "digest": "two-digest-multiple-changed",
              "size": 321
            },
            "diffStatus": "changed"
          }
        ]
        "###);

        // 7. Check added resources (external)
        assert_json_snapshot!(diff_map.get("http://localhost/three"), @r###"
        [
          {
            "url": "http://localhost/three",
            "content": {
              "digest": "three-digest-added",
              "size": 321
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 8. Check added resources (internal)
        assert_json_snapshot!(diff_map.get("three-digest-inline-added"), @r###"
        [
          {
            "content": {
              "digest": "three-digest-inline-added",
              "size": 123
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 9. Check added resources (external, multiple)
        assert_json_snapshot!(diff_map.get("http://localhost/three-multiple"), @r###"
        [
          {
            "url": "http://localhost/three-multiple",
            "content": {
              "digest": "three-digest-multiple-added",
              "size": 321
            },
            "diffStatus": "added"
          },
          {
            "url": "http://localhost/three-multiple",
            "content": {
              "digest": "three-digest-multiple-added",
              "size": 321
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 10. Check added resources (internal, multiple)
        assert_json_snapshot!(diff_map.get("three-digest-multiple-inline-added"), @r###"
        [
          {
            "content": {
              "digest": "three-digest-multiple-inline-added",
              "size": 123
            },
            "diffStatus": "added"
          },
          {
            "content": {
              "digest": "three-digest-multiple-inline-added",
              "size": 123
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 11. Check removed resources (external)
        assert_json_snapshot!(diff_map.get("http://localhost/three-removed"), @r###"
        [
          {
            "url": "http://localhost/three-removed",
            "content": {
              "digest": "three-digest-removed",
              "size": 321
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 12. Check removed resources (internal)
        assert_json_snapshot!(diff_map.get("three-digest-inline-removed"), @r###"
        [
          {
            "content": {
              "digest": "three-digest-inline-removed",
              "size": 123
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 13. Check removed resources (external, multiple)
        assert_json_snapshot!(diff_map.get("http://localhost/three-multiple-removed"), @r###"
        [
          {
            "url": "http://localhost/three-multiple-removed",
            "content": {
              "digest": "three-digest-multiple-removed",
              "size": 321
            },
            "diffStatus": "removed"
          },
          {
            "url": "http://localhost/three-multiple-removed",
            "content": {
              "digest": "three-digest-multiple-removed",
              "size": 321
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 14. Check removed resources (internal, multiple)
        assert_json_snapshot!(diff_map.get("three-digest-multiple-inline-removed"), @r###"
        [
          {
            "content": {
              "digest": "three-digest-multiple-inline-removed",
              "size": 123
            },
            "diffStatus": "removed"
          },
          {
            "content": {
              "digest": "three-digest-multiple-inline-removed",
              "size": 123
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        Ok(())
    }

    #[test]
    fn correctly_calculates_web_page_revisions_diff() -> anyhow::Result<()> {
        let resource_one_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content("one-digest-no-change", 123)
                .build();
        let resource_two_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest", 321)
                .build();
        let resource_three_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-removed")?)
                .set_content("three-digest-removed", 321)
                .build();
        let resource_four_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/four")?)
                .set_content("four-digest", 321)
                .build();

        let resource_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content("one-digest-no-change", 123)
                .build();
        let resource_two_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest-changed", 321)
                .build();
        let resource_three_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content("three-digest-added", 321)
                .build();
        let resource_four_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/four")?)
                .set_content("four-digest-changed", 321)
                .build();

        let resource_one_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/five")?)
                .set_content("five-digest-added", 123)
                .build();
        let resource_two_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content("two-digest-changed", 321)
                .build();
        let resource_three_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content("three-digest-changed", 321)
                .build();

        let diff = web_page_resources_revisions_diff(vec![
            WebPageResourcesRevision {
                timestamp: OffsetDateTime::from_unix_timestamp(946720100)?,
                scripts: vec![resource_one_rev_1, resource_two_rev_1, resource_three_rev_1],
                styles: vec![resource_four_rev_1],
            },
            WebPageResourcesRevision {
                timestamp: OffsetDateTime::from_unix_timestamp(946720200)?,
                scripts: vec![resource_one_rev_2, resource_two_rev_2, resource_three_rev_2],
                styles: vec![resource_four_rev_2],
            },
            WebPageResourcesRevision {
                timestamp: OffsetDateTime::from_unix_timestamp(946720300)?,
                scripts: vec![resource_one_rev_3, resource_two_rev_3, resource_three_rev_3],
                styles: vec![],
            },
        ]);

        assert_eq!(diff.len(), 3);

        assert_json_snapshot!(diff[0], { ".scripts" => insta::sorted_redaction() }, @r###"
        {
          "timestamp": 946720100,
          "scripts": [
            {
              "url": "http://localhost/one",
              "content": {
                "digest": "one-digest-no-change",
                "size": 123
              }
            },
            {
              "url": "http://localhost/three-removed",
              "content": {
                "digest": "three-digest-removed",
                "size": 321
              }
            },
            {
              "url": "http://localhost/two",
              "content": {
                "digest": "two-digest",
                "size": 321
              }
            }
          ],
          "styles": [
            {
              "url": "http://localhost/four",
              "content": {
                "digest": "four-digest",
                "size": 321
              }
            }
          ]
        }
        "###);
        assert_json_snapshot!(diff[1], { ".scripts" => insta::sorted_redaction() }, @r###"
        {
          "timestamp": 946720200,
          "scripts": [
            {
              "url": "http://localhost/one",
              "content": {
                "digest": "one-digest-no-change",
                "size": 123
              }
            },
            {
              "url": "http://localhost/three",
              "content": {
                "digest": "three-digest-added",
                "size": 321
              },
              "diffStatus": "added"
            },
            {
              "url": "http://localhost/three-removed",
              "content": {
                "digest": "three-digest-removed",
                "size": 321
              },
              "diffStatus": "removed"
            },
            {
              "url": "http://localhost/two",
              "content": {
                "digest": "two-digest-changed",
                "size": 321
              },
              "diffStatus": "changed"
            }
          ],
          "styles": [
            {
              "url": "http://localhost/four",
              "content": {
                "digest": "four-digest-changed",
                "size": 321
              },
              "diffStatus": "changed"
            }
          ]
        }
        "###);
        assert_json_snapshot!(diff[2], { ".scripts" => insta::sorted_redaction() }, @r###"
        {
          "timestamp": 946720300,
          "scripts": [
            {
              "url": "http://localhost/five",
              "content": {
                "digest": "five-digest-added",
                "size": 123
              },
              "diffStatus": "added"
            },
            {
              "url": "http://localhost/one",
              "content": {
                "digest": "one-digest-no-change",
                "size": 123
              },
              "diffStatus": "removed"
            },
            {
              "url": "http://localhost/three",
              "content": {
                "digest": "three-digest-changed",
                "size": 321
              },
              "diffStatus": "changed"
            },
            {
              "url": "http://localhost/two",
              "content": {
                "digest": "two-digest-changed",
                "size": 321
              }
            }
          ],
          "styles": [
            {
              "url": "http://localhost/four",
              "content": {
                "digest": "four-digest-changed",
                "size": 321
              },
              "diffStatus": "removed"
            }
          ]
        }
        "###);

        Ok(())
    }
}
