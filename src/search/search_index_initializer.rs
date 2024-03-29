use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    search::{SearchFilter, SearchItem},
    utils::Util,
};
use std::collections::HashMap;
use time::OffsetDateTime;

const UTIL_SEARCH_CATEGORY: &str = "Utils";

/// Flattens a vector of util nodes into a `util-id <-> util` map.
fn flatten_utils_tree(utils: Vec<Util>, utils_map: &mut HashMap<i32, Util>) {
    for mut util in utils {
        if let Some(child_utils) = util.utils.take() {
            flatten_utils_tree(child_utils, utils_map);
        }

        utils_map.insert(util.id, util);
    }
}

/// Checks if we should re-index util definition in the search index.
fn is_update_needed(util: &Util, searchable_util: &SearchItem) -> bool {
    let util_handle = if let Some(handle) = searchable_util
        .meta
        .as_ref()
        .and_then(|meta| meta.get("handle"))
    {
        handle
    } else {
        return true;
    };

    if util.name != searchable_util.label || util_handle != &util.handle {
        return true;
    }

    match (&searchable_util.keywords, &util.keywords) {
        (Some(searchable_keywords), Some(keywords)) => searchable_keywords != keywords,
        (None, Some(keywords)) => !keywords.is_empty(),
        (_, None) => false,
    }
}

/// Converts instance of `Util` to `SearchItem`.
fn util_to_search_item(util: Util, timestamp: OffsetDateTime) -> SearchItem {
    SearchItem {
        id: SearchItem::create_id(&util.name, UTIL_SEARCH_CATEGORY, None, None),
        label: util.name,
        keywords: util.keywords,
        category: UTIL_SEARCH_CATEGORY.to_string(),
        sub_category: None,
        user_id: None,
        timestamp,
        meta: Some(
            [
                ("id".to_string(), util.id.to_string()),
                ("handle".to_string(), util.handle),
            ]
            .into_iter()
            .collect(),
        ),
    }
}

/// Initialize search index that powers Secutils app wide search.
pub async fn populate_search_index<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
) -> anyhow::Result<()> {
    // Flatten utils tree to a map.
    let mut utils = HashMap::new();
    flatten_utils_tree(api.utils().get_all().await?, &mut utils);

    log::debug!("Found {} utils.", utils.len());

    let search_api = api.search();
    let searchable_utils =
        search_api.search(SearchFilter::default().with_category(UTIL_SEARCH_CATEGORY))?;
    let mut utils_indexed = 0;
    for searchable_util in searchable_utils {
        let util = match searchable_util
            .meta
            .as_ref()
            .and_then(|meta| Some(meta.get("id")?.parse::<i32>()))
        {
            Some(Ok(util_id)) => utils.remove(&util_id),
            None | Some(_) => {
                // Util has invalid definition and must be deleted.
                log::error!(
                    "Invalid search item found for util and will be removed: {:?}",
                    searchable_util
                );
                search_api.remove(searchable_util.id)?;

                None
            }
        };

        // Update changed util definition or remove non-existent util definition.
        match util {
            Some(util) if util.keywords.is_some() => {
                if is_update_needed(&util, &searchable_util) {
                    let updated_searchable_util =
                        util_to_search_item(util, OffsetDateTime::now_utc());
                    log::debug!(
                        "Search item for util needs to be updated: from {:?} to {:?}",
                        searchable_util,
                        updated_searchable_util
                    );
                    search_api.upsert(updated_searchable_util)?;
                }
                utils_indexed += 1;
            }
            Some(_) | None => {
                log::debug!(
                    "Non-existent search item found for util and will be removed: {:?}",
                    searchable_util
                );
                search_api.remove(searchable_util.id)?;
            }
        }
    }

    // Insert new util definitions.
    for util in utils.into_values().filter(|util| util.keywords.is_some()) {
        search_api.upsert(util_to_search_item(util, OffsetDateTime::now_utc()))?;
        utils_indexed += 1;
    }

    log::debug!("Indexed {} utils.", utils_indexed);

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        search::{
            search_index_initializer::{
                flatten_utils_tree, is_update_needed, util_to_search_item, UTIL_SEARCH_CATEGORY,
            },
            SearchItem,
        },
        utils::Util,
    };
    use insta::assert_json_snapshot;
    use std::collections::HashMap;
    use time::OffsetDateTime;

    #[test]
    fn can_flatten_utils_tree() -> anyhow::Result<()> {
        let utils = vec![
            Util {
                id: 1,
                handle: "handle-1".to_string(),
                name: "name-1".to_string(),
                keywords: Some("keywords-1".to_string()),
                utils: Some(vec![Util {
                    id: 2,
                    handle: "handle-1-2".to_string(),
                    name: "name-1-2".to_string(),
                    keywords: Some("keywords-1-2".to_string()),
                    utils: None,
                }]),
            },
            Util {
                id: 3,
                handle: "handle-3".to_string(),
                name: "name-3".to_string(),
                keywords: Some("keywords-3".to_string()),
                utils: None,
            },
        ];

        let mut utils_map = HashMap::new();
        flatten_utils_tree(utils, &mut utils_map);
        insta::with_settings!({ sort_maps => true }, {
            assert_json_snapshot!(utils_map, @r###"
            {
              "1": {
                "handle": "handle-1",
                "name": "name-1"
              },
              "2": {
                "handle": "handle-1-2",
                "name": "name-1-2"
              },
              "3": {
                "handle": "handle-3",
                "name": "name-3"
              }
            }
            "###);
        });

        Ok(())
    }

    #[test]
    fn can_detect_if_update_is_needed() -> anyhow::Result<()> {
        let util = Util {
            id: 1,
            handle: "handle-1".to_string(),
            name: "name-1".to_string(),
            keywords: Some("keywords-1".to_string()),
            utils: None,
        };

        // Name/label mismatch.
        assert!(is_update_needed(
            &util,
            &SearchItem {
                id: 1,
                label: "name-1-new".to_string(),
                keywords: Some("keywords-1".to_string()),
                category: UTIL_SEARCH_CATEGORY.to_string(),
                sub_category: None,
                user_id: None,
                meta: Some(
                    [("handle".to_string(), "handle-1".to_string())]
                        .into_iter()
                        .collect()
                ),
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        ));

        // Keywords mismatch.
        assert!(is_update_needed(
            &util,
            &SearchItem {
                id: 1,
                label: "name-1".to_string(),
                keywords: Some("keywords-1-new".to_string()),
                category: UTIL_SEARCH_CATEGORY.to_string(),
                sub_category: None,
                user_id: None,
                meta: Some(
                    [("handle".to_string(), "handle-1".to_string())]
                        .into_iter()
                        .collect()
                ),
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        ));

        // Handle mismatch.
        assert!(is_update_needed(
            &util,
            &SearchItem {
                id: 1,
                label: "name-1".to_string(),
                keywords: Some("keywords-1".to_string()),
                category: UTIL_SEARCH_CATEGORY.to_string(),
                sub_category: None,
                user_id: None,
                meta: Some(
                    [("handle".to_string(), "handle-1-new".to_string())]
                        .into_iter()
                        .collect()
                ),
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        ));

        // Everything is equal.
        assert!(!is_update_needed(
            &util,
            &SearchItem {
                id: 1,
                label: "name-1".to_string(),
                keywords: Some("keywords-1".to_string()),
                category: UTIL_SEARCH_CATEGORY.to_string(),
                sub_category: None,
                user_id: None,
                meta: Some(
                    [("handle".to_string(), "handle-1".to_string())]
                        .into_iter()
                        .collect()
                ),
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        ));

        Ok(())
    }

    #[test]
    fn can_convert_to_searchable_item() -> anyhow::Result<()> {
        let timestamp = OffsetDateTime::from_unix_timestamp(946720800)?;
        assert_eq!(
            util_to_search_item(
                Util {
                    id: 1,
                    handle: "handle-1".to_string(),
                    name: "name-1".to_string(),
                    keywords: Some("keywords-1".to_string()),
                    utils: None,
                },
                timestamp
            ),
            SearchItem {
                id: 13488033034572884071,
                label: "name-1".to_string(),
                keywords: Some("keywords-1".to_string()),
                category: UTIL_SEARCH_CATEGORY.to_string(),
                sub_category: None,
                user_id: None,
                meta: Some(
                    [
                        ("id".to_string(), "1".to_string()),
                        ("handle".to_string(), "handle-1".to_string())
                    ]
                    .into_iter()
                    .collect()
                ),
                timestamp,
            }
        );

        Ok(())
    }
}
