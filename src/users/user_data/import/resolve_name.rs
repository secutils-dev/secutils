use crate::{
    users::{
        scripts::api_ext::MAX_SCRIPT_NAME_LENGTH,
        secrets::api_ext::MAX_SECRET_NAME_LENGTH,
        tags::user_tag::{MAX_TAG_NAME_LENGTH, normalize_tag_name},
        user_data::import::{ConflictResolution, ImportEntitySelection},
    },
    utils::MAX_ENTITY_NAME_LENGTH,
};
use std::collections::HashSet;

/// Name constraints applied by each imported entity's creation API.
#[derive(Clone, Copy)]
pub enum ImportNameKind {
    Entity,
    Script,
    Secret,
    Tag,
}

/// Resolves the name for an imported entity based on conflict resolution,
/// keeping generated copies unique and within the entity's name constraints.
pub fn resolve_name(
    original_name: &str,
    selection: Option<&&ImportEntitySelection>,
    existing_names: &HashSet<String>,
    kind: ImportNameKind,
) -> String {
    if selection.is_some_and(|s| s.conflict_resolution == Some(ConflictResolution::Rename)) {
        generate_copy_name(original_name, existing_names, kind)
    } else {
        original_name.to_string()
    }
}

/// Reserves room for the suffix before checking uniqueness, without splitting UTF-8 characters.
fn generate_copy_name(
    name: &str,
    existing_names: &HashSet<String>,
    kind: ImportNameKind,
) -> String {
    let max_length = match kind {
        ImportNameKind::Entity => MAX_ENTITY_NAME_LENGTH,
        ImportNameKind::Script => MAX_SCRIPT_NAME_LENGTH,
        ImportNameKind::Secret => MAX_SECRET_NAME_LENGTH,
        ImportNameKind::Tag => MAX_TAG_NAME_LENGTH,
    };
    let normalized_name;
    let name = if matches!(kind, ImportNameKind::Tag) {
        normalized_name = normalize_tag_name(name);
        normalized_name.as_str()
    } else {
        name
    };
    for n in 1u64.. {
        let suffix = match kind {
            ImportNameKind::Secret => format!("_COPY_{n}"),
            ImportNameKind::Tag => format!(" (copy {n})"),
            _ => format!(" (Copy {n})"),
        };
        let mut end = name.len().min(max_length - suffix.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{suffix}", &name[..end]);
        if !existing_names.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("All copy name suffixes exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_copy_name_first_copy() {
        let existing = HashSet::new();
        assert_eq!(
            generate_copy_name("foo", &existing, ImportNameKind::Entity),
            "foo (Copy 1)"
        );
    }

    #[test]
    fn generate_copy_name_increments_when_copy_exists() {
        let existing: HashSet<String> = ["foo (Copy 1)".to_string()].into();
        assert_eq!(
            generate_copy_name("foo", &existing, ImportNameKind::Entity),
            "foo (Copy 2)"
        );
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
        let result = resolve_name("foo", Some(&sel_ref), &existing, ImportNameKind::Entity);
        assert_eq!(result, "foo (Copy 1)");
    }

    #[test]
    fn copy_names_respect_each_entity_length_limit() {
        for (kind, limit, suffix) in [
            (ImportNameKind::Entity, MAX_ENTITY_NAME_LENGTH, " (Copy 1)"),
            (ImportNameKind::Script, MAX_SCRIPT_NAME_LENGTH, " (Copy 1)"),
            (ImportNameKind::Secret, MAX_SECRET_NAME_LENGTH, "_COPY_1"),
            (ImportNameKind::Tag, MAX_TAG_NAME_LENGTH, " (copy 1)"),
        ] {
            let name = "a".repeat(limit);
            let copy = generate_copy_name(&name, &HashSet::new(), kind);
            assert_eq!(copy.len(), limit);
            assert_eq!(
                copy,
                format!("{}{suffix}", "a".repeat(limit - suffix.len()))
            );
        }
    }

    #[test]
    fn copy_names_truncate_at_utf8_boundaries() {
        let name = "界".repeat(33);
        let copy = generate_copy_name(&name, &HashSet::new(), ImportNameKind::Entity);
        assert_eq!(copy, format!("{} (Copy 1)", "界".repeat(30)));
        assert!(copy.len() <= MAX_ENTITY_NAME_LENGTH);
    }

    #[test]
    fn secret_copy_names_are_valid_and_unique_after_truncation() {
        let name = "A".repeat(MAX_SECRET_NAME_LENGTH);
        let mut existing = HashSet::new();
        for n in 1..=10 {
            let copy = generate_copy_name(&name, &existing, ImportNameKind::Secret);
            let suffix = format!("_COPY_{n}");
            assert_eq!(
                copy,
                format!(
                    "{}{suffix}",
                    "A".repeat(MAX_SECRET_NAME_LENGTH - suffix.len())
                )
            );
            assert!(copy.starts_with('A'));
            assert!(copy.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            assert!(existing.insert(copy));
        }
    }

    #[test]
    fn tag_copy_names_check_uniqueness_after_normalization() {
        let existing = HashSet::from(["alpha (copy 1)".to_string()]);
        assert_eq!(
            generate_copy_name(" ALPHA ", &existing, ImportNameKind::Tag),
            "alpha (copy 2)"
        );
    }
}
