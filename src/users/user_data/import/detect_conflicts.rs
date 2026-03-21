use super::results::ImportConflict;
use std::collections::HashMap;
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
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
