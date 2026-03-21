use super::results::ApplyDeleteItem;
use std::collections::HashSet;
use uuid::Uuid;

/// Detects items in existing data that are not in the import file (for Apply mode deletion).
pub fn detect_deletions(
    import_names: &[&str],
    existing_items: &[(Uuid, &str)],
) -> Vec<ApplyDeleteItem> {
    let import_names_set: HashSet<&str> = import_names.iter().copied().collect();
    existing_items
        .iter()
        .filter(|(_, name)| !import_names_set.contains(name))
        .map(|(id, name)| ApplyDeleteItem {
            id: *id,
            name: name.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_deletions_finds_missing_items() {
        let import_names = vec!["script-a"];
        let existing = vec![
            (Uuid::from_u128(1), "script-a"),
            (Uuid::from_u128(2), "script-b"),
        ];
        let deletions = detect_deletions(&import_names, &existing);
        assert_eq!(deletions.len(), 1);
        assert_eq!(deletions[0].name, "script-b");
    }

    #[test]
    fn detect_deletions_returns_empty_when_all_present() {
        let import_names = vec!["script-a", "script-b"];
        let existing = vec![
            (Uuid::from_u128(1), "script-a"),
            (Uuid::from_u128(2), "script-b"),
        ];
        let deletions = detect_deletions(&import_names, &existing);
        assert!(deletions.is_empty());
    }
}
