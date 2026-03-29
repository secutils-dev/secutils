use crate::users::UserId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Named badge color values for tags (EUI palette names, kept for backward compatibility).
const NAMED_COLORS: &[&str] = &[
    "default", "primary", "success", "accent", "warning", "danger",
];

/// Maximum length for a tag name.
pub const MAX_TAG_NAME_LENGTH: usize = 50;

/// Maximum number of tags a user may create.
pub const MAX_TAGS_PER_USER: usize = 100;

/// Maximum number of tags that can be assigned to a single entity.
#[allow(dead_code)]
pub const MAX_TAGS_PER_ENTITY: usize = 10;

/// A user-managed tag with a name and display color.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserTag {
    pub id: Uuid,
    #[serde(skip, default = "default_user_id")]
    pub user_id: UserId,
    pub name: String,
    pub color: String,
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub updated_at: OffsetDateTime,
}

fn default_user_id() -> UserId {
    UserId::from(Uuid::nil())
}

/// Normalizes a tag name: trims whitespace and lowercases.
pub fn normalize_tag_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Returns `true` if the color is a named EUI palette color or a valid hex color (#RGB or #RRGGBB).
pub fn is_valid_tag_color(color: &str) -> bool {
    if NAMED_COLORS.contains(&color) {
        return true;
    }

    let bytes = color.as_bytes();
    matches!(bytes.len(), 4 | 7)
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// Returns `true` if the tag name is non-empty and within the length limit after normalization.
pub fn is_valid_tag_name(name: &str) -> bool {
    let normalized = normalize_tag_name(name);
    !normalized.is_empty() && normalized.len() <= MAX_TAG_NAME_LENGTH
}

/// Slim tag representation embedded in entity API responses and entity-level
/// export data. Contains only the fields the UI needs to render a tag badge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityTag {
    pub id: Uuid,
    pub name: String,
    pub color: String,
}

impl From<UserTag> for EntityTag {
    fn from(tag: UserTag) -> Self {
        Self {
            id: tag.id,
            name: tag.name,
            color: tag.color,
        }
    }
}

// Tag pointer.
impl From<Uuid> for EntityTag {
    fn from(id: Uuid) -> Self {
        Self {
            id,
            name: String::new(),
            color: String::new(),
        }
    }
}

/// Raw row returned by entity-tag JOIN queries. Defined once here so every
/// `database_ext` module can reuse it with `query_as!`.
#[derive(Debug)]
pub struct RawEntityTag {
    pub entity_id: Uuid,
    pub id: Uuid,
    pub name: String,
    pub color: String,
}

impl From<RawEntityTag> for EntityTag {
    fn from(raw: RawEntityTag) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            color: raw.color,
        }
    }
}

/// Groups a flat list of entity-tag rows into a map from entity ID to its tags.
pub fn group_entity_tags(rows: Vec<RawEntityTag>) -> HashMap<Uuid, Vec<EntityTag>> {
    let mut map: HashMap<Uuid, Vec<EntityTag>> = HashMap::new();
    for row in rows {
        let entity_id = row.entity_id;
        map.entry(entity_id).or_default().push(row.into());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_tag_name_trims_and_lowercases() {
        assert_eq!(normalize_tag_name("  Production  "), "production");
        assert_eq!(normalize_tag_name("My Tag"), "my tag");
        assert_eq!(normalize_tag_name(""), "");
        assert_eq!(normalize_tag_name("   "), "");
    }

    #[test]
    fn is_valid_tag_color_accepts_named_colors() {
        for color in [
            "default", "primary", "success", "accent", "warning", "danger",
        ] {
            assert!(is_valid_tag_color(color), "Expected '{color}' to be valid");
        }
    }

    #[test]
    fn is_valid_tag_color_accepts_hex_colors() {
        assert!(is_valid_tag_color("#000"));
        assert!(is_valid_tag_color("#fff"));
        assert!(is_valid_tag_color("#F0F"));
        assert!(is_valid_tag_color("#000000"));
        assert!(is_valid_tag_color("#ffffff"));
        assert!(is_valid_tag_color("#54B399"));
        assert!(is_valid_tag_color("#6092C0"));
    }

    #[test]
    fn is_valid_tag_color_rejects_invalid_colors() {
        assert!(!is_valid_tag_color("red"));
        assert!(!is_valid_tag_color(""));
        assert!(!is_valid_tag_color("blue"));
        assert!(!is_valid_tag_color("#"));
        assert!(!is_valid_tag_color("#GG"));
        assert!(!is_valid_tag_color("#12"));
        assert!(!is_valid_tag_color("#12345"));
        assert!(!is_valid_tag_color("#1234567"));
        assert!(!is_valid_tag_color("000000"));
    }

    #[test]
    fn is_valid_tag_name_rejects_empty_or_long_names() {
        assert!(!is_valid_tag_name(""));
        assert!(!is_valid_tag_name("   "));
        assert!(!is_valid_tag_name(&"a".repeat(MAX_TAG_NAME_LENGTH + 1)));
    }

    #[test]
    fn is_valid_tag_name_accepts_valid_names() {
        assert!(is_valid_tag_name("production"));
        assert!(is_valid_tag_name("  staging  "));
        assert!(is_valid_tag_name(&"a".repeat(MAX_TAG_NAME_LENGTH)));
    }

    #[test]
    fn user_tag_serialization_excludes_user_id() {
        let tag = UserTag {
            id: Uuid::nil(),
            user_id: UserId::from(Uuid::nil()),
            name: "test".to_string(),
            color: "primary".to_string(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let json = serde_json::to_value(&tag).unwrap();
        assert!(json.get("userId").is_none());
        assert_eq!(json["name"], "test");
        assert_eq!(json["color"], "primary");
    }
}
