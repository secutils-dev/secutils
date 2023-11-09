use crate::utils::{
    WebPageDataRevision, WebPageResource, WebPageResourceContentData, WebPageResourceDiffStatus,
    WebPageResourcesData, WebPageResourcesTrackerTag,
};
use anyhow::anyhow;
use itertools::{EitherOrBoth, Itertools};
use std::{collections::HashMap, str::FromStr};
use tlsh2::Tlsh;

/// Parameters used by the Web Scraper: checksum length - 1, TLS hash length - 72 bytes, and code
/// size - 32 bytes.
type TlshDefault = Tlsh<1, 72, 32>;

/// While comparing TLS hashes, we treat resources different if their size differs by more than 10%.
const TLSH_CONTENT_SIZE_THRESHOLD_PERCENT: f32 = 10.0;

/// While comparing TLS hashes, we treat resources different if their distance is greater than 200.
const TLSH_DISTANCE_THRESHOLD: i32 = 200;

struct WebPageResourcesDiffMap {
    resources: HashMap<String, Vec<WebPageResource>>,
    similarity_hashes: Vec<(String, usize)>,
}

/// Takes multiple web page resources revisions and updates diff status for resources in the
/// adjacent revisions.
pub fn web_page_resources_revisions_diff(
    revisions: Vec<WebPageDataRevision<WebPageResourcesTrackerTag>>,
) -> anyhow::Result<Vec<WebPageDataRevision<WebPageResourcesTrackerTag>>> {
    // We can only calculate diff if there are at least two revisions.
    if revisions.len() < 2 {
        return Ok(revisions);
    }

    let mut revisions_diff = Vec::with_capacity(revisions.len());
    let mut peekable_revisions = revisions.into_iter().rev().peekable();
    while let Some(current_revision) = peekable_revisions.next() {
        if let Some(previous_revision) = peekable_revisions.peek() {
            revisions_diff.push(WebPageDataRevision {
                id: current_revision.id,
                tracker_id: current_revision.tracker_id,
                created_at: current_revision.created_at,
                data: WebPageResourcesData {
                    scripts: web_page_resources_diff(
                        previous_revision.data.scripts.clone(),
                        current_revision.data.scripts,
                    )?,
                    styles: web_page_resources_diff(
                        previous_revision.data.styles.clone(),
                        current_revision.data.styles,
                    )?,
                },
            });
        } else {
            revisions_diff.push(current_revision);
        }
    }

    Ok(revisions_diff.into_iter().rev().collect())
}

/// Takes two sets of resources - current and previous revision - and returns a set of resources
/// with a populated diff status: added, removed, or changed.
fn web_page_resources_diff(
    resources_from: Vec<WebPageResource>,
    resources_to: Vec<WebPageResource>,
) -> anyhow::Result<Vec<WebPageResource>> {
    // Most of the time resources don't change, so it makes sense to use original length as capacity.
    let mut resources_diff = Vec::with_capacity(resources_to.len());

    // Takes pair of resources from current and previous version and updates diff status.
    let update_resource_status = |resource_pair: EitherOrBoth<WebPageResource, WebPageResource>| {
        match resource_pair {
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
        }
    };

    let mut resources_from_map = web_page_resources_to_map(resources_from);
    let resources_to_map = web_page_resources_to_map(resources_to);
    for (resource_key, resources_to) in resources_to_map.resources {
        if let Some(resources_from) = resources_from_map.resources.remove(resource_key.as_str()) {
            resources_diff.extend(
                resources_to
                    .into_iter()
                    .zip_longest(resources_from.into_iter())
                    .map(update_resource_status),
            )
        } else {
            // We use similarity search only for inline resources with TLS hash as we don't have any
            // other comparable identifier like URLs for the external resources.
            let hasher_and_size = resources_to
                .get(0)
                .and_then(|resource| {
                    if resource.is_external_resource() {
                        return None;
                    }

                    let content = resource.content.as_ref()?;
                    if let WebPageResourceContentData::Tlsh(ref hash) = content.data {
                        Some(
                            TlshDefault::from_str(hash)
                                .map(|hasher| (hasher, content.size))
                                .map_err(|_| anyhow!("Cannot parse TLS hash: {:?}", hash)),
                        )
                    } else {
                        None
                    }
                })
                .transpose()?;

            // Check if there are resources that can be compared by similarity hashes, otherwise
            // mark all resources as added.
            if let Some((hasher_to, size_to)) = hasher_and_size {
                let mut hash_and_distance: Option<(&str, i32)> = None;

                // Find the most similar resource in the previous revision (with smaller TLSH distance).
                for (hash_from, size_from) in &resources_from_map.similarity_hashes {
                    // The similarity hashes are sorted by size, so we are iterating only until
                    // resources with a significantly bigger size.
                    let absolute_diff = size_to.abs_diff(*size_from) as f32;
                    let average_size = (size_to + size_from) as f32 / 2.0;
                    if absolute_diff / average_size * 100.0 > TLSH_CONTENT_SIZE_THRESHOLD_PERCENT {
                        if size_from > &size_to {
                            break;
                        } else {
                            continue;
                        }
                    }

                    // Skip resources that are not similar enough.
                    let hasher_from = TlshDefault::from_str(hash_from)
                        .map_err(|_| anyhow!("Cannot parse TLS hash: {:?}", hash_from))?;

                    let distance = hasher_from.diff(&hasher_to, true);
                    let lowest_known_distance = hash_and_distance
                        .map(|(_, d)| d)
                        .unwrap_or(TLSH_DISTANCE_THRESHOLD);
                    if distance < lowest_known_distance
                        && resources_from_map.resources.contains_key(hash_from)
                    {
                        hash_and_distance.replace((hash_from.as_str(), distance));
                    }
                }

                resources_diff.extend(
                    resources_to
                        .into_iter()
                        .zip_longest(
                            hash_and_distance
                                .and_then(|(hash_from, _)| {
                                    resources_from_map.resources.remove(hash_from)
                                })
                                .into_iter()
                                .flatten(),
                        )
                        .map(update_resource_status),
                );
            } else {
                resources_diff.extend(resources_to.into_iter().map(|added_resource| {
                    added_resource.with_diff_status(WebPageResourceDiffStatus::Added)
                }));
            }
        }
    }

    // Add resources that were removed, i.e. exist in the `resources_from` but not in the `resources_to`.
    resources_diff.extend(resources_from_map.resources.into_values().flatten().map(
        |removed_resource| removed_resource.with_diff_status(WebPageResourceDiffStatus::Removed),
    ));

    Ok(resources_diff)
}

/// Adds resources to a map where key is either URL (external resource) or data digest (inline resource).
fn web_page_resources_to_map(resources: Vec<WebPageResource>) -> WebPageResourcesDiffMap {
    let mut exact_match_resources_map = HashMap::new();
    let mut similarity_hashes = vec![];
    for resource in resources {
        let resource_key = match (&resource.url, &resource.content) {
            (Some(url), _) => url.to_string(),
            (_, Some(content)) => {
                if let WebPageResourceContentData::Tlsh(ref value) = content.data {
                    similarity_hashes.push((value.to_string(), content.size));
                }

                content.data.value().to_string()
            }
            _ => {
                log::warn!("Resource is missing both URL and content: {:?}", resource);
                continue;
            }
        };

        exact_match_resources_map
            .entry(resource_key)
            .or_insert_with(Vec::new)
            .push(resource);
    }

    // Sort by size to make search more efficient.
    similarity_hashes.sort_by(|(_, size_a), (_, size_b)| size_a.cmp(size_b));

    WebPageResourcesDiffMap {
        resources: exact_match_resources_map,
        similarity_hashes,
    }
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
            WebPageDataRevision, WebPageResourceContentData, WebPageResourcesData,
        },
    };
    use insta::assert_json_snapshot;
    use std::str::FromStr;
    use time::OffsetDateTime;
    use tlsh2::{Tlsh, TlshDefaultBuilder};
    use url::Url;
    use uuid::uuid;

    #[test]
    fn tls_hash_properly_calculated() -> anyhow::Result<()> {
        let test_raw_data_a = "alert(1);alert(2);alert(3);alert(4);alert(5);console.log(6);";
        let test_hash_a =
            "T172A0021519C40C242F86775C090C100124801A5170435C46500D52FE00557F2807D114";
        let tls_hash_a_from_hash = Tlsh::<1, 72, 32>::from_str(test_hash_a).unwrap();
        let tls_hash_a_from_content =
            TlshDefaultBuilder::build_from(test_raw_data_a.as_bytes()).unwrap();

        let test_raw_data_b = "window.document.body.innerHTML = \"Hello Secutils.dev and world!\";";
        let test_hash_b =
            "T156A002B39256197413252E602EA57AC67D66540474113459D79DB004B1608C7C8EEEDD";
        let tls_hash_b_from_hash = Tlsh::<1, 72, 32>::from_str(test_hash_b).unwrap();
        let tls_hash_b_from_content =
            TlshDefaultBuilder::build_from(test_raw_data_b.as_bytes()).unwrap();

        let test_raw_data_c = "alert(1);alert(2);alert(4);alert(3);alert(5);console.log(6);";
        let test_hash_c =
            "T102A0021519C40C242F86775C090C100124801A5170435C46500D52FE00557F2807D114";
        let tls_hash_c_from_hash = Tlsh::<1, 72, 32>::from_str(test_hash_c).unwrap();
        let tls_hash_c_from_content =
            TlshDefaultBuilder::build_from(test_raw_data_c.as_bytes()).unwrap();

        assert_eq!(
            String::from_utf8(tls_hash_a_from_hash.hash().to_vec())?,
            test_hash_a,
        );
        assert_eq!(
            String::from_utf8(tls_hash_a_from_content.hash().to_vec())?,
            test_hash_a,
        );

        assert_eq!(
            String::from_utf8(tls_hash_b_from_hash.hash().to_vec())?,
            test_hash_b,
        );
        assert_eq!(
            String::from_utf8(tls_hash_b_from_content.hash().to_vec())?,
            test_hash_b,
        );

        assert_eq!(
            String::from_utf8(tls_hash_c_from_hash.hash().to_vec())?,
            test_hash_c,
        );
        assert_eq!(
            String::from_utf8(tls_hash_c_from_content.hash().to_vec())?,
            test_hash_c,
        );

        assert_eq!(tls_hash_a_from_content.diff(&tls_hash_a_from_hash, true), 0);
        assert_eq!(
            tls_hash_a_from_content.diff(&tls_hash_c_from_content, true),
            1
        );
        assert_eq!(tls_hash_a_from_hash.diff(&tls_hash_b_from_hash, true), 188);
        assert_eq!(tls_hash_a_from_content.diff(&tls_hash_c_from_hash, true), 1);
        assert_eq!(tls_hash_a_from_hash.diff(&tls_hash_c_from_hash, true), 1);
        assert_eq!(tls_hash_a_from_hash.diff(&tls_hash_c_from_content, true), 1);

        Ok(())
    }

    #[test]
    fn correctly_collects_resources_to_map() -> anyhow::Result<()> {
        let resource_one =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest".to_string()),
                    123,
                )
                .build();
        let resource_two =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest".to_string()),
                    321,
                )
                .build();
        let resource_three = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest".to_string()),
            456,
        )
        .build();
        let resource_four =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest".to_string()),
                    321,
                )
                .build();
        let resource_five = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("four-digest".to_string()),
            456,
        )
        .build();
        let resource_six = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest".to_string()),
            456,
        )
        .build();
        let resource_seven = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Raw("seven-raw".to_string()),
            789,
        )
        .build();
        let resource_eight = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Tlsh("eight-hash".to_string()),
            456,
        )
        .build();
        let resource_nine = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Tlsh("nine-hash".to_string()),
            789,
        )
        .build();

        let resources_map = web_page_resources_to_map(vec![
            resource_one.clone(),
            resource_two.clone(),
            resource_three.clone(),
            resource_four.clone(),
            resource_five.clone(),
            resource_six.clone(),
            resource_seven.clone(),
            resource_eight,
            resource_nine,
        ]);
        assert_eq!(resources_map.resources.len(), 7);
        assert_eq!(resources_map.similarity_hashes.len(), 2);
        assert_eq!(
            resources_map.resources.get("http://localhost/one"),
            Some(&vec![resource_one])
        );
        assert_eq!(
            resources_map.resources.get("http://localhost/two"),
            Some(&vec![resource_two, resource_four])
        );
        assert_eq!(
            resources_map.resources.get("three-digest"),
            Some(&vec![resource_three, resource_six])
        );
        assert_eq!(
            resources_map.resources.get("four-digest"),
            Some(&vec![resource_five])
        );
        assert_eq!(
            resources_map.resources.get("seven-raw"),
            Some(&vec![resource_seven])
        );
        assert_eq!(
            resources_map.similarity_hashes,
            vec![
                ("eight-hash".to_string(), 456),
                ("nine-hash".to_string(), 789)
            ]
        );

        Ok(())
    }

    #[test]
    fn correctly_calculates_web_page_resources_diff() -> anyhow::Result<()> {
        // Not changed resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        // 5. Inline changed with TLS hash
        let resource_one_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-no-change".to_string()),
                    123,
                )
                .build();
        let resource_one_inline_rev_1 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("one-digest-inline-no-change".to_string()),
            123,
        )
        .build();
        let resource_one_multiple_one_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one-multiple")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-multiple-no-change".to_string()),
                    123,
                )
                .build();
        let resource_one_multiple_inline_rev_1 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("one-digest-multiple-inline-no-change".to_string()),
            123,
        )
        .build();

        // Changed resources:
        // 1. External, single resource
        // 2. External, multiple resources
        let resource_two_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest".to_string()),
                    321,
                )
                .build();
        let resource_two_multiple_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two-multiple")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest-multiple".to_string()),
                    321,
                )
                .build();
        let resource_two_tlsh_inline_rev_1 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Tlsh(
                "T172A0021519C40C242F86775C090C100124801A5170435C46500D52FE00557F2807D114"
                    .to_string(),
            ),
            123,
        )
        .build();

        // Removed resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        let resource_three_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-removed")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-removed".to_string()),
                    321,
                )
                .build();
        let resource_three_inline_rev_1 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest-inline-removed".to_string()),
            123,
        )
        .build();
        let resource_three_multiple_rev_1 = MockWebPageResourceBuilder::with_url(Url::parse(
            "http://localhost/three-multiple-removed",
        )?)
        .set_content(
            WebPageResourceContentData::Sha1("three-digest-multiple-removed".to_string()),
            321,
        )
        .build();
        let resource_three_multiple_inline_rev_1 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest-multiple-inline-removed".to_string()),
            123,
        )
        .build();

        let resource_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-no-change".to_string()),
                    123,
                )
                .build();
        let resource_one_inline_rev_2 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("one-digest-inline-no-change".to_string()),
            123,
        )
        .build();
        let resource_one_multiple_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one-multiple")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-multiple-no-change".to_string()),
                    123,
                )
                .build();
        let resource_one_multiple_inline_rev_2 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("one-digest-multiple-inline-no-change".to_string()),
            123,
        )
        .build();

        let resource_two_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest-changed".to_string()),
                    321,
                )
                .build();
        let resource_two_multiple_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two-multiple")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest-multiple-changed".to_string()),
                    321,
                )
                .build();
        let resource_two_tlsh_inline_rev_2 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Tlsh(
                "T16EA0020625914D256FCEFB48581C10163480057470935CDE900D52FC00457F1013E450"
                    .to_string(),
            ),
            123,
        )
        .build();

        // Added resources:
        // 1. External, single resource
        // 2. Inline, single resource
        // 3. External, multiple resources
        // 4. Inline, multiple resources
        let resource_three_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-added".to_string()),
                    321,
                )
                .build();
        let resource_three_inline_rev_2 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest-inline-added".to_string()),
            123,
        )
        .build();
        let resource_three_multiple_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-multiple")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-multiple-added".to_string()),
                    321,
                )
                .build();
        let resource_three_multiple_inline_rev_2 = MockWebPageResourceBuilder::with_content(
            WebPageResourceContentData::Sha1("three-digest-multiple-inline-added".to_string()),
            123,
        )
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
                resource_two_tlsh_inline_rev_1,
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
                resource_two_tlsh_inline_rev_2,
                resource_three_rev_2,
                resource_three_inline_rev_2,
                resource_three_multiple_rev_2.clone(),
                resource_three_multiple_rev_2,
                resource_three_multiple_inline_rev_2.clone(),
                resource_three_multiple_inline_rev_2,
            ],
        )?;

        let diff_map = web_page_resources_to_map(diff);
        assert_eq!(diff_map.resources.len(), 15);
        assert_eq!(diff_map.similarity_hashes.len(), 1);

        // 1. Check not changed resources (external)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/one"), @r###"
        [
          {
            "url": "http://localhost/one",
            "content": {
              "data": {
                "sha1": "one-digest-no-change"
              },
              "size": 123
            }
          }
        ]
        "###);

        // 2. Check not changed resources (internal)
        assert_json_snapshot!(diff_map.resources.get("one-digest-inline-no-change"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "one-digest-inline-no-change"
              },
              "size": 123
            }
          }
        ]
        "###);

        // 3. Check not changed resources (external, multiple)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/one-multiple"), @r###"
        [
          {
            "url": "http://localhost/one-multiple",
            "content": {
              "data": {
                "sha1": "one-digest-multiple-no-change"
              },
              "size": 123
            }
          },
          {
            "url": "http://localhost/one-multiple",
            "content": {
              "data": {
                "sha1": "one-digest-multiple-no-change"
              },
              "size": 123
            }
          }
        ]
        "###);

        // 4. Check not changed resources (internal, multiple)
        assert_json_snapshot!(diff_map.resources.get("one-digest-multiple-inline-no-change"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "one-digest-multiple-inline-no-change"
              },
              "size": 123
            }
          },
          {
            "content": {
              "data": {
                "sha1": "one-digest-multiple-inline-no-change"
              },
              "size": 123
            }
          }
        ]
        "###);

        // 5. Check changed resources (external)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/two"), @r###"
        [
          {
            "url": "http://localhost/two",
            "content": {
              "data": {
                "sha1": "two-digest-changed"
              },
              "size": 321
            },
            "diffStatus": "changed"
          }
        ]
        "###);

        // 6. Check changed resources (external, multiple)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/two-multiple"), @r###"
        [
          {
            "url": "http://localhost/two-multiple",
            "content": {
              "data": {
                "sha1": "two-digest-multiple"
              },
              "size": 321
            }
          },
          {
            "url": "http://localhost/two-multiple",
            "content": {
              "data": {
                "sha1": "two-digest-multiple-changed"
              },
              "size": 321
            },
            "diffStatus": "changed"
          }
        ]
        "###);

        // 7. Check changed resources (inline with TLS hash)
        assert_json_snapshot!(diff_map.resources.get("T16EA0020625914D256FCEFB48581C10163480057470935CDE900D52FC00457F1013E450"), @r###"
        [
          {
            "content": {
              "data": {
                "tlsh": "T16EA0020625914D256FCEFB48581C10163480057470935CDE900D52FC00457F1013E450"
              },
              "size": 123
            },
            "diffStatus": "changed"
          }
        ]
        "###);

        // 8. Check added resources (external)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/three"), @r###"
        [
          {
            "url": "http://localhost/three",
            "content": {
              "data": {
                "sha1": "three-digest-added"
              },
              "size": 321
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 9. Check added resources (internal)
        assert_json_snapshot!(diff_map.resources.get("three-digest-inline-added"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "three-digest-inline-added"
              },
              "size": 123
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 10. Check added resources (external, multiple)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/three-multiple"), @r###"
        [
          {
            "url": "http://localhost/three-multiple",
            "content": {
              "data": {
                "sha1": "three-digest-multiple-added"
              },
              "size": 321
            },
            "diffStatus": "added"
          },
          {
            "url": "http://localhost/three-multiple",
            "content": {
              "data": {
                "sha1": "three-digest-multiple-added"
              },
              "size": 321
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 11. Check added resources (internal, multiple)
        assert_json_snapshot!(diff_map.resources.get("three-digest-multiple-inline-added"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "three-digest-multiple-inline-added"
              },
              "size": 123
            },
            "diffStatus": "added"
          },
          {
            "content": {
              "data": {
                "sha1": "three-digest-multiple-inline-added"
              },
              "size": 123
            },
            "diffStatus": "added"
          }
        ]
        "###);

        // 12. Check removed resources (external)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/three-removed"), @r###"
        [
          {
            "url": "http://localhost/three-removed",
            "content": {
              "data": {
                "sha1": "three-digest-removed"
              },
              "size": 321
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 13. Check removed resources (internal)
        assert_json_snapshot!(diff_map.resources.get("three-digest-inline-removed"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "three-digest-inline-removed"
              },
              "size": 123
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 14. Check removed resources (external, multiple)
        assert_json_snapshot!(diff_map.resources.get("http://localhost/three-multiple-removed"), @r###"
        [
          {
            "url": "http://localhost/three-multiple-removed",
            "content": {
              "data": {
                "sha1": "three-digest-multiple-removed"
              },
              "size": 321
            },
            "diffStatus": "removed"
          },
          {
            "url": "http://localhost/three-multiple-removed",
            "content": {
              "data": {
                "sha1": "three-digest-multiple-removed"
              },
              "size": 321
            },
            "diffStatus": "removed"
          }
        ]
        "###);

        // 15. Check removed resources (internal, multiple)
        assert_json_snapshot!(diff_map.resources.get("three-digest-multiple-inline-removed"), @r###"
        [
          {
            "content": {
              "data": {
                "sha1": "three-digest-multiple-inline-removed"
              },
              "size": 123
            },
            "diffStatus": "removed"
          },
          {
            "content": {
              "data": {
                "sha1": "three-digest-multiple-inline-removed"
              },
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
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-no-change".to_string()),
                    123,
                )
                .build();
        let resource_two_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest".to_string()),
                    321,
                )
                .build();
        let resource_three_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three-removed")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-removed".to_string()),
                    321,
                )
                .build();
        let resource_four_rev_1 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/four")?)
                .set_content(
                    WebPageResourceContentData::Sha1("four-digest".to_string()),
                    321,
                )
                .build();

        let resource_one_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/one")?)
                .set_content(
                    WebPageResourceContentData::Sha1("one-digest-no-change".to_string()),
                    123,
                )
                .build();
        let resource_two_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest-changed".to_string()),
                    321,
                )
                .build();
        let resource_three_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-added".to_string()),
                    321,
                )
                .build();
        let resource_four_rev_2 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/four")?)
                .set_content(
                    WebPageResourceContentData::Sha1("four-digest-changed".to_string()),
                    321,
                )
                .build();

        let resource_one_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/five")?)
                .set_content(
                    WebPageResourceContentData::Sha1("five-digest-added".to_string()),
                    123,
                )
                .build();
        let resource_two_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/two")?)
                .set_content(
                    WebPageResourceContentData::Sha1("two-digest-changed".to_string()),
                    321,
                )
                .build();
        let resource_three_rev_3 =
            MockWebPageResourceBuilder::with_url(Url::parse("http://localhost/three")?)
                .set_content(
                    WebPageResourceContentData::Sha1("three-digest-changed".to_string()),
                    321,
                )
                .build();

        let diff = web_page_resources_revisions_diff(vec![
            WebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                created_at: OffsetDateTime::from_unix_timestamp(946720100)?,
                data: WebPageResourcesData {
                    scripts: vec![resource_one_rev_1, resource_two_rev_1, resource_three_rev_1],
                    styles: vec![resource_four_rev_1],
                },
            },
            WebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000011"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                created_at: OffsetDateTime::from_unix_timestamp(946720200)?,
                data: WebPageResourcesData {
                    scripts: vec![resource_one_rev_2, resource_two_rev_2, resource_three_rev_2],
                    styles: vec![resource_four_rev_2],
                },
            },
            WebPageDataRevision {
                id: uuid!("00000000-0000-0000-0000-000000000021"),
                tracker_id: uuid!("00000000-0000-0000-0000-000000000002"),
                created_at: OffsetDateTime::from_unix_timestamp(946720300)?,
                data: WebPageResourcesData {
                    scripts: vec![resource_one_rev_3, resource_two_rev_3, resource_three_rev_3],
                    styles: vec![],
                },
            },
        ])?;

        assert_eq!(diff.len(), 3);

        assert_json_snapshot!(diff[0], { ".data.scripts" => insta::sorted_redaction() }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "data": {
            "scripts": [
              {
                "url": "http://localhost/one",
                "content": {
                  "data": {
                    "sha1": "one-digest-no-change"
                  },
                  "size": 123
                }
              },
              {
                "url": "http://localhost/three-removed",
                "content": {
                  "data": {
                    "sha1": "three-digest-removed"
                  },
                  "size": 321
                }
              },
              {
                "url": "http://localhost/two",
                "content": {
                  "data": {
                    "sha1": "two-digest"
                  },
                  "size": 321
                }
              }
            ],
            "styles": [
              {
                "url": "http://localhost/four",
                "content": {
                  "data": {
                    "sha1": "four-digest"
                  },
                  "size": 321
                }
              }
            ]
          },
          "createdAt": 946720100
        }
        "###);
        assert_json_snapshot!(diff[1], { ".data.scripts" => insta::sorted_redaction() }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000011",
          "data": {
            "scripts": [
              {
                "url": "http://localhost/one",
                "content": {
                  "data": {
                    "sha1": "one-digest-no-change"
                  },
                  "size": 123
                }
              },
              {
                "url": "http://localhost/three",
                "content": {
                  "data": {
                    "sha1": "three-digest-added"
                  },
                  "size": 321
                },
                "diffStatus": "added"
              },
              {
                "url": "http://localhost/three-removed",
                "content": {
                  "data": {
                    "sha1": "three-digest-removed"
                  },
                  "size": 321
                },
                "diffStatus": "removed"
              },
              {
                "url": "http://localhost/two",
                "content": {
                  "data": {
                    "sha1": "two-digest-changed"
                  },
                  "size": 321
                },
                "diffStatus": "changed"
              }
            ],
            "styles": [
              {
                "url": "http://localhost/four",
                "content": {
                  "data": {
                    "sha1": "four-digest-changed"
                  },
                  "size": 321
                },
                "diffStatus": "changed"
              }
            ]
          },
          "createdAt": 946720200
        }
        "###);
        assert_json_snapshot!(diff[2], { ".data.scripts" => insta::sorted_redaction() }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000021",
          "data": {
            "scripts": [
              {
                "url": "http://localhost/five",
                "content": {
                  "data": {
                    "sha1": "five-digest-added"
                  },
                  "size": 123
                },
                "diffStatus": "added"
              },
              {
                "url": "http://localhost/one",
                "content": {
                  "data": {
                    "sha1": "one-digest-no-change"
                  },
                  "size": 123
                },
                "diffStatus": "removed"
              },
              {
                "url": "http://localhost/three",
                "content": {
                  "data": {
                    "sha1": "three-digest-changed"
                  },
                  "size": 321
                },
                "diffStatus": "changed"
              },
              {
                "url": "http://localhost/two",
                "content": {
                  "data": {
                    "sha1": "two-digest-changed"
                  },
                  "size": 321
                }
              }
            ],
            "styles": [
              {
                "url": "http://localhost/four",
                "content": {
                  "data": {
                    "sha1": "four-digest-changed"
                  },
                  "size": 321
                },
                "diffStatus": "removed"
              }
            ]
          },
          "createdAt": 946720300
        }
        "###);

        Ok(())
    }
}
