use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{
        EntityTag, TagCreateParams, User, UserTag,
        user_data::import::{
            ConflictResolution, ImportEntityResult, ImportEntitySelection, resolve_name,
        },
    },
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Imports tag definitions from the export file.
///
/// For each tag: if a tag with the same name already exists for this user, the
/// conflict resolution from the selection is applied (rename / overwrite / skip).
/// If no conflict, the tag is created. If no selection is provided for a tag,
/// conflicts default to mapping to the existing tag (backward-compat).
///
/// Returns a mapping from old (exported) tag ID to new (imported) tag ID, plus
/// import result counters.
pub async fn import_tags<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user: &User,
    file_tags: &[UserTag],
    selections: &HashMap<Uuid, &ImportEntitySelection>,
) -> (HashMap<Uuid, Uuid>, ImportEntityResult) {
    let mut result = ImportEntityResult::default();
    let mut tag_id_map: HashMap<Uuid, Uuid> = HashMap::new();

    if file_tags.is_empty() {
        return (tag_id_map, result);
    }

    let tags_api = api.tags(user);
    let existing_tags = tags_api.list_tags().await.unwrap_or_default();
    let existing_by_name: HashMap<_, _> =
        existing_tags.iter().map(|t| (t.name.as_str(), t)).collect();
    let mut used_names: HashSet<String> = existing_tags.iter().map(|t| t.name.clone()).collect();

    for file_tag in file_tags {
        let selection = selections.get(&file_tag.id);

        if let Some(existing) = existing_by_name.get(file_tag.name.as_str()) {
            // Name conflict — apply conflict resolution.
            let resolution = selection.and_then(|s| s.conflict_resolution);
            match resolution {
                Some(ConflictResolution::Rename) => {
                    // Create with a unique copy name.
                    match tags_api
                        .create_tag(TagCreateParams {
                            name: resolve_name(&file_tag.name, selection, &used_names),
                            color: file_tag.color.clone(),
                        })
                        .await
                    {
                        Ok(new_tag) => {
                            tag_id_map.insert(file_tag.id, new_tag.id);
                            // The API normalizes tag names to lowercase.
                            used_names.insert(new_tag.name.clone());
                            result.imported += 1;
                        }
                        Err(err) => {
                            result.failed += 1;
                            result
                                .errors
                                .push(format!("Tag '{}': {err}", file_tag.name));
                        }
                    }
                }
                Some(ConflictResolution::Skip) => {
                    result.skipped += 1;
                }
                // Overwrite or no selection: map to existing tag.
                _ => {
                    tag_id_map.insert(file_tag.id, existing.id);
                    result.skipped += 1;
                }
            }
        } else {
            // No conflict — create new tag.
            match tags_api
                .create_tag(TagCreateParams {
                    name: file_tag.name.clone(),
                    color: file_tag.color.clone(),
                })
                .await
            {
                Ok(new_tag) => {
                    tag_id_map.insert(file_tag.id, new_tag.id);
                    used_names.insert(new_tag.name.clone());
                    result.imported += 1;
                }
                Err(err) => {
                    result.failed += 1;
                    result
                        .errors
                        .push(format!("Tag '{}': {err}", file_tag.name));
                }
            }
        }
    }

    (tag_id_map, result)
}

/// Extracts old tag IDs from entity-level tag objects and remaps them to new IDs.
/// Returns only the successfully mapped IDs.
pub fn remap_tag_ids(entity_tags: &[EntityTag], tag_id_map: &HashMap<Uuid, Uuid>) -> Vec<Uuid> {
    entity_tags
        .iter()
        .filter_map(|t| tag_id_map.get(&t.id).copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{import_tags, remap_tag_ids};
    use crate::{
        tests::{mock_api_with_config, mock_config, mock_user},
        users::{
            EntityTag, TagCreateParams, UserId, UserTag,
            user_data::import::{ConflictResolution, ImportAction, ImportEntitySelection},
        },
    };
    use sqlx::PgPool;
    use std::collections::HashMap;
    use time::macros::datetime;
    use uuid::Uuid;

    fn make_file_tag(name: &str, color: &str) -> UserTag {
        UserTag {
            id: Uuid::now_v7(),
            user_id: UserId::from(Uuid::nil()),
            name: name.to_string(),
            color: color.to_string(),
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }
    }

    #[sqlx::test]
    async fn import_tags_empty(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let (tag_id_map, result) = import_tags(&api, &user, &[], &HashMap::new()).await;
        assert!(tag_id_map.is_empty());
        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn import_tags_creates_new(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        let tag_a = make_file_tag("alpha", "#54B399");
        let tag_b = make_file_tag("beta", "#6092C0");
        let old_id_a = tag_a.id;
        let old_id_b = tag_b.id;

        let (tag_id_map, result) = import_tags(&api, &user, &[tag_a, tag_b], &HashMap::new()).await;
        assert_eq!(result.imported, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(tag_id_map.len(), 2);

        // Verify tags exist in DB.
        let db_tags = api.tags(&user).list_tags().await?;
        assert_eq!(db_tags.len(), 2);

        // Verify mapping points to valid new IDs (different from old).
        let new_id_a = tag_id_map[&old_id_a];
        let new_id_b = tag_id_map[&old_id_b];
        assert!(
            db_tags
                .iter()
                .any(|t| t.id == new_id_a && t.name == "alpha")
        );
        assert!(db_tags.iter().any(|t| t.id == new_id_b && t.name == "beta"));

        Ok(())
    }

    #[sqlx::test]
    async fn import_tags_skips_existing_by_name(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Pre-create a tag.
        let existing = api
            .tags(&user)
            .create_tag(TagCreateParams {
                name: "alpha".to_string(),
                color: "#54B399".to_string(),
            })
            .await?;

        // Import a tag with the same name but different ID and color.
        let file_tag = make_file_tag("alpha", "#E7664C");
        let old_id = file_tag.id;

        let (tag_id_map, result) = import_tags(&api, &user, &[file_tag], &HashMap::new()).await;
        assert_eq!(result.skipped, 1);
        assert_eq!(result.imported, 0);
        assert_eq!(result.failed, 0);

        // Mapping should point to the existing tag.
        assert_eq!(tag_id_map[&old_id], existing.id);

        // DB should still have only one tag.
        let db_tags = api.tags(&user).list_tags().await?;
        assert_eq!(db_tags.len(), 1);

        Ok(())
    }

    #[sqlx::test]
    async fn import_tags_mixed_new_and_existing(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Pre-create one tag.
        let existing = api
            .tags(&user)
            .create_tag(TagCreateParams {
                name: "alpha".to_string(),
                color: "#54B399".to_string(),
            })
            .await?;

        let file_tag_existing = make_file_tag("alpha", "#E7664C");
        let file_tag_new = make_file_tag("beta", "#6092C0");
        let old_id_existing = file_tag_existing.id;
        let old_id_new = file_tag_new.id;

        let (tag_id_map, result) = import_tags(
            &api,
            &user,
            &[file_tag_existing, file_tag_new],
            &HashMap::new(),
        )
        .await;
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 0);
        assert_eq!(tag_id_map.len(), 2);

        assert_eq!(tag_id_map[&old_id_existing], existing.id);
        assert_ne!(tag_id_map[&old_id_new], old_id_new);

        let db_tags = api.tags(&user).list_tags().await?;
        assert_eq!(db_tags.len(), 2);

        Ok(())
    }

    #[sqlx::test]
    async fn import_tags_rename_on_conflict(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Pre-create a tag with the same name.
        api.tags(&user)
            .create_tag(TagCreateParams {
                name: "alpha".to_string(),
                color: "#54B399".to_string(),
            })
            .await?;

        // Import a tag with the same name, using Rename conflict resolution.
        let file_tag = make_file_tag("alpha", "#E7664C");
        let old_id = file_tag.id;

        let sel = ImportEntitySelection {
            source_id: old_id,
            action: ImportAction::Import,
            conflict_resolution: Some(ConflictResolution::Rename),
        };
        let selections = HashMap::from([(old_id, &sel)]);

        let (tag_id_map, result) = import_tags(&api, &user, &[file_tag], &selections).await;
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.failed, 0);

        // Mapping should point to the newly created tag.
        let new_id = tag_id_map[&old_id];

        // DB should have 2 tags: "alpha" and "alpha (Copy 1)".
        let db_tags = api.tags(&user).list_tags().await?;
        assert_eq!(db_tags.len(), 2);
        assert!(db_tags.iter().any(|t| t.name == "alpha"));
        assert!(
            db_tags
                .iter()
                .any(|t| t.id == new_id && t.name == "alpha (copy 1)")
        );

        Ok(())
    }

    #[sqlx::test]
    async fn import_tags_skip_on_conflict(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api_with_config(pool, mock_config()?).await?;
        let user = mock_user()?;
        api.db.upsert_user(&user).await?;

        // Pre-create a tag with the same name.
        api.tags(&user)
            .create_tag(TagCreateParams {
                name: "alpha".to_string(),
                color: "#54B399".to_string(),
            })
            .await?;

        // Import a tag with the same name, using Skip conflict resolution.
        let file_tag = make_file_tag("alpha", "#E7664C");
        let old_id = file_tag.id;

        let sel = ImportEntitySelection {
            source_id: old_id,
            action: ImportAction::Import,
            conflict_resolution: Some(ConflictResolution::Skip),
        };
        let selections = HashMap::from([(old_id, &sel)]);

        let (tag_id_map, result) = import_tags(&api, &user, &[file_tag], &selections).await;
        assert_eq!(result.skipped, 1);
        assert_eq!(result.imported, 0);

        // Tag should NOT be in the map (skipped entirely).
        assert!(tag_id_map.is_empty());

        // DB should still have only one tag.
        let db_tags = api.tags(&user).list_tags().await?;
        assert_eq!(db_tags.len(), 1);

        Ok(())
    }

    #[test]
    fn remap_tag_ids_basic() {
        let old_a = Uuid::now_v7();
        let old_b = Uuid::now_v7();
        let new_a = Uuid::now_v7();
        let new_b = Uuid::now_v7();

        let mut map = HashMap::new();
        map.insert(old_a, new_a);
        map.insert(old_b, new_b);

        let entity_tags = vec![
            EntityTag {
                id: old_a,
                name: "a".to_string(),
                color: "#000".to_string(),
            },
            EntityTag {
                id: old_b,
                name: "b".to_string(),
                color: "#111".to_string(),
            },
        ];

        let result = remap_tag_ids(&entity_tags, &map);
        assert_eq!(result, vec![new_a, new_b]);
    }

    #[test]
    fn remap_tag_ids_skips_unmapped() {
        let old_a = Uuid::now_v7();
        let old_b = Uuid::now_v7();
        let new_a = Uuid::now_v7();

        let mut map = HashMap::new();
        map.insert(old_a, new_a);
        // old_b is NOT in the map.

        let entity_tags = vec![
            EntityTag {
                id: old_a,
                name: "a".to_string(),
                color: "#000".to_string(),
            },
            EntityTag {
                id: old_b,
                name: "b".to_string(),
                color: "#111".to_string(),
            },
        ];

        let result = remap_tag_ids(&entity_tags, &map);
        assert_eq!(result, vec![new_a]);
    }

    #[test]
    fn remap_tag_ids_empty() {
        let map = HashMap::new();
        assert!(remap_tag_ids(&[], &map).is_empty());

        let mut map_with_entry = HashMap::new();
        map_with_entry.insert(Uuid::now_v7(), Uuid::now_v7());
        assert!(remap_tag_ids(&[], &map_with_entry).is_empty());
    }
}
