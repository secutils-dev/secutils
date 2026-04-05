use super::results::ImportConflict;
use crate::utils::webhooks::{Responder, ResponderLocation, ResponderMethod};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Detects conflicts between named entities in the import file and existing user data.
pub fn detect_conflicts(
    import_items: &[(Uuid, &str)],
    existing_items: &[(Uuid, &str)],
) -> Vec<ImportConflict> {
    let existing_by_name: HashMap<&str, Uuid> = existing_items
        .iter()
        .map(|(id, name)| (*name, *id))
        .collect();
    import_items
        .iter()
        .filter_map(|(source_id, name)| {
            existing_by_name
                .get(name)
                .map(|existing_id| ImportConflict {
                    source_id: *source_id,
                    name: name.to_string(),
                    existing_id: *existing_id,
                    rename_allowed: true,
                })
        })
        .collect()
}

/// Returns true if two methods conflict (same method, or either is `Any`).
fn methods_conflict(a: ResponderMethod, b: ResponderMethod) -> bool {
    a == b || a == ResponderMethod::Any || b == ResponderMethod::Any
}

/// Returns the string key used for location-based conflict comparison.
/// When `strip_subdomain_prefix` is true and the location carries a prefix, the prefix
/// portion is omitted so that imported responders with unsupported prefixes are compared
/// as if they had no prefix.
fn location_key(location: &ResponderLocation, strip_subdomain_prefix: bool) -> String {
    if strip_subdomain_prefix && location.subdomain_prefix.is_some() {
        format!(
            ":{}:{}",
            location.path_type,
            location.path.to_ascii_lowercase()
        )
    } else {
        location.to_string()
    }
}

/// Detects responder conflicts by name **and** location+method.
///
/// A conflict is produced when an imported responder matches an existing one by:
/// - Same name, OR
/// - Same location AND conflicting method (equal, or either is `Any`)
///
/// If both criteria match the **same** existing responder, only one conflict is emitted.
///
/// When `strip_subdomain_prefix` is true, subdomain prefixes on **imported** responder
/// locations are ignored during comparison (existing responders never carry unsupported
/// prefixes since the server validated them at creation time).
pub fn detect_responder_conflicts(
    import_items: &[&Responder],
    existing_items: &[&Responder],
    strip_subdomain_prefix: bool,
) -> Vec<ImportConflict> {
    let existing_by_name: HashMap<&str, &Responder> = existing_items
        .iter()
        .map(|r| (r.name.as_str(), *r))
        .collect();

    // Index existing responders by location string so we only check
    // location+method conflicts against responders that share the same path,
    // instead of scanning the entire existing list for every import.
    // Existing responders are never stripped - they were validated at creation time.
    let mut existing_by_location: HashMap<String, Vec<&Responder>> =
        HashMap::with_capacity(existing_items.len());
    for &r in existing_items {
        existing_by_location
            .entry(location_key(&r.location, false))
            .or_default()
            .push(r);
    }

    let mut conflicts = Vec::new();
    for &imported in import_items {
        let mut seen_ids: HashSet<Uuid> = HashSet::new();
        let import_loc = location_key(&imported.location, strip_subdomain_prefix);

        // Check name conflict.
        if let Some(existing) = existing_by_name.get(imported.name.as_str()) {
            seen_ids.insert(existing.id);
            // If the same existing responder also conflicts on location+method,
            // renaming won't help - the location+method collision remains.
            let also_location_conflict = location_key(&existing.location, false) == import_loc
                && methods_conflict(imported.method, existing.method);
            conflicts.push(ImportConflict {
                source_id: imported.id,
                name: imported.name.clone(),
                existing_id: existing.id,
                rename_allowed: !also_location_conflict,
            });
        }

        // Check location+method conflict only against responders at the same location.
        if let Some(same_location) = existing_by_location.get(&import_loc) {
            for &existing in same_location {
                if seen_ids.contains(&existing.id) {
                    continue;
                }
                if methods_conflict(imported.method, existing.method) {
                    seen_ids.insert(existing.id);
                    conflicts.push(ImportConflict {
                        source_id: imported.id,
                        name: imported.name.clone(),
                        existing_id: existing.id,
                        rename_allowed: false,
                    });
                }
            }
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::webhooks::{ResponderLocation, ResponderPathType, ResponderSettings};
    use time::macros::datetime;

    #[test]
    fn detect_conflicts_finds_name_matches() {
        let import_items = vec![(Uuid::nil(), "script-a"), (Uuid::from_u128(1), "script-b")];
        let existing_items = vec![(Uuid::from_u128(100), "script-b")];
        let conflicts = detect_conflicts(&import_items, &existing_items);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].name, "script-b");
        assert_eq!(conflicts[0].source_id, Uuid::from_u128(1));
        assert_eq!(conflicts[0].existing_id, Uuid::from_u128(100));
    }

    #[test]
    fn detect_conflicts_returns_empty_when_no_matches() {
        let import_items = vec![(Uuid::nil(), "script-a")];
        let existing_items = vec![(Uuid::from_u128(100), "script-b")];
        let conflicts = detect_conflicts(&import_items, &existing_items);
        assert!(conflicts.is_empty());
    }

    fn make_responder(id: u128, name: &str, path: &str, method: ResponderMethod) -> Responder {
        make_responder_with_prefix(id, name, path, method, None)
    }

    fn make_responder_with_prefix(
        id: u128,
        name: &str,
        path: &str,
        method: ResponderMethod,
        subdomain_prefix: Option<&str>,
    ) -> Responder {
        Responder {
            id: Uuid::from_u128(id),
            name: name.to_string(),
            location: ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: path.to_string(),
                subdomain_prefix: subdomain_prefix.map(str::to_string),
            },
            method,
            enabled: true,
            settings: ResponderSettings {
                requests_to_track: 1,
                status_code: 200,
                body: None,
                headers: None,
                script: None,
                secrets: crate::users::SecretsAccess::None,
            },
            tags: vec![],
            created_at: datetime!(2020-01-01 00:00:00 UTC),
            updated_at: datetime!(2020-01-01 00:00:00 UTC),
        }
    }

    #[test]
    fn responder_conflicts_finds_location_method_match() {
        let imported = make_responder(1, "new-name", "/test", ResponderMethod::Get);
        let existing = make_responder(100, "old-name", "/test", ResponderMethod::Get);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].source_id, Uuid::from_u128(1));
        assert_eq!(conflicts[0].existing_id, Uuid::from_u128(100));
        assert!(!conflicts[0].rename_allowed);
    }

    #[test]
    fn responder_conflicts_any_method_conflicts_with_specific() {
        let imported = make_responder(1, "new-name", "/test", ResponderMethod::Any);
        let existing = make_responder(100, "old-name", "/test", ResponderMethod::Get);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert_eq!(conflicts.len(), 1);

        // And the reverse.
        let imported2 = make_responder(2, "new-name2", "/test", ResponderMethod::Post);
        let existing2 = make_responder(200, "old-name2", "/test", ResponderMethod::Any);
        let conflicts2 = detect_responder_conflicts(&[&imported2], &[&existing2], false);
        assert_eq!(conflicts2.len(), 1);
    }

    #[test]
    fn responder_conflicts_deduplicates_name_and_location() {
        // Same name AND same location+method → only one conflict, rename NOT allowed
        // because renaming won't resolve the location+method collision.
        let imported = make_responder(1, "same-name", "/test", ResponderMethod::Get);
        let existing = make_responder(100, "same-name", "/test", ResponderMethod::Get);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert_eq!(conflicts.len(), 1);
        assert!(!conflicts[0].rename_allowed);
    }

    #[test]
    fn responder_conflicts_name_only_allows_rename() {
        // Same name but different location → rename IS allowed.
        let imported = make_responder(1, "same-name", "/path-a", ResponderMethod::Get);
        let existing = make_responder(100, "same-name", "/path-b", ResponderMethod::Post);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].rename_allowed);
    }

    #[test]
    fn responder_conflicts_name_and_location_match_different_existing() {
        // Name matches one existing, location+method matches a different one → two conflicts.
        let imported = make_responder(1, "resp-a", "/path-b", ResponderMethod::Get);
        let existing_name = make_responder(100, "resp-a", "/other", ResponderMethod::Post);
        let existing_loc = make_responder(200, "resp-b", "/path-b", ResponderMethod::Get);
        let conflicts =
            detect_responder_conflicts(&[&imported], &[&existing_name, &existing_loc], false);
        assert_eq!(conflicts.len(), 2);
        // Name conflict allows rename, location conflict does not.
        let name_conflict = conflicts
            .iter()
            .find(|c| c.existing_id == Uuid::from_u128(100))
            .unwrap();
        assert!(name_conflict.rename_allowed);
        let loc_conflict = conflicts
            .iter()
            .find(|c| c.existing_id == Uuid::from_u128(200))
            .unwrap();
        assert!(!loc_conflict.rename_allowed);
    }

    #[test]
    fn responder_conflicts_no_match_different_path() {
        let imported = make_responder(1, "new", "/path-a", ResponderMethod::Get);
        let existing = make_responder(100, "old", "/path-b", ResponderMethod::Get);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn responder_conflicts_no_match_different_method() {
        let imported = make_responder(1, "new", "/test", ResponderMethod::Get);
        let existing = make_responder(100, "old", "/test", ResponderMethod::Post);
        let conflicts = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn strip_prefix_makes_imported_match_existing_without_prefix() {
        // Imported has prefix "abc", existing has no prefix — same path+method.
        // Without stripping they don't conflict; with stripping they do.
        let imported =
            make_responder_with_prefix(1, "new", "/test", ResponderMethod::Get, Some("abc"));
        let existing = make_responder(100, "old", "/test", ResponderMethod::Get);

        let without_strip = detect_responder_conflicts(&[&imported], &[&existing], false);
        assert!(without_strip.is_empty());

        let with_strip = detect_responder_conflicts(&[&imported], &[&existing], true);
        assert_eq!(with_strip.len(), 1);
        assert!(!with_strip[0].rename_allowed);
    }

    #[test]
    fn strip_prefix_no_effect_when_imported_has_no_prefix() {
        let imported = make_responder(1, "new", "/test", ResponderMethod::Get);
        let existing = make_responder(100, "old", "/test", ResponderMethod::Get);
        let without_strip = detect_responder_conflicts(&[&imported], &[&existing], false);
        let with_strip = detect_responder_conflicts(&[&imported], &[&existing], true);
        assert_eq!(without_strip.len(), with_strip.len());
    }

    #[test]
    fn strip_prefix_collapses_different_prefixes_to_same_location() {
        // Two imports with different prefixes but same path — after stripping both
        // conflict with the same existing responder.
        let imported_a =
            make_responder_with_prefix(1, "a", "/test", ResponderMethod::Get, Some("x"));
        let imported_b =
            make_responder_with_prefix(2, "b", "/test", ResponderMethod::Get, Some("y"));
        let existing = make_responder(100, "old", "/test", ResponderMethod::Get);

        let conflicts = detect_responder_conflicts(&[&imported_a, &imported_b], &[&existing], true);
        assert_eq!(conflicts.len(), 2);
    }
}
