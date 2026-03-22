use crate::users::user_data::import::{ConflictResolution, ImportEntitySelection};
use std::collections::HashSet;

/// Resolves the name for an imported entity based on conflict resolution,
/// guaranteeing uniqueness against a set of existing names using the `(Copy N)` pattern.
pub fn resolve_name(
    original_name: &str,
    selection: Option<&&ImportEntitySelection>,
    existing_names: &HashSet<String>,
) -> String {
    if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Rename)) {
        generate_copy_name(original_name, existing_names)
    } else {
        original_name.to_string()
    }
}

/// Generates a unique copy name by appending `(Copy N)` where N is auto-incremented.
/// Matches the frontend `getCopyName` pattern.
fn generate_copy_name(name: &str, existing_names: &HashSet<String>) -> String {
    let mut candidate = format!("{name} (Copy 1)");
    let mut n = 1u32;
    while existing_names.contains(&candidate) {
        n += 1;
        candidate = format!("{name} (Copy {n})");
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_copy_name_first_copy() {
        let existing = HashSet::new();
        assert_eq!(generate_copy_name("foo", &existing), "foo (Copy 1)");
    }

    #[test]
    fn generate_copy_name_increments_when_copy_exists() {
        let existing: HashSet<String> = ["foo (Copy 1)".to_string()].into();
        assert_eq!(generate_copy_name("foo", &existing), "foo (Copy 2)");
    }

    #[test]
    fn resolve_name_returns_copy_when_rename_resolution() {
        use super::super::params::{ConflictResolution, ImportAction, ImportEntitySelection};
        use uuid::Uuid;
        let sel = ImportEntitySelection {
            source_id: Uuid::nil(),
            action: ImportAction::Import,
            conflict_resolution: Some(ConflictResolution::Rename),
        };
        let existing: HashSet<String> = HashSet::new();
        let sel_ref = &sel;
        let result = resolve_name("foo", Some(&sel_ref), &existing);
        assert_eq!(result, "foo (Copy 1)");
    }
}
