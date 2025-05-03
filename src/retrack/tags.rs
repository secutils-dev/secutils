use std::iter::once;

/// Defines a tag for the Secutils application trackers that also serves as a prefix for any other
/// Secutils custom tags.
const RETRACK_APP_TAG: &str = "secutils";

/// Defines a tag that stores user ID.
pub const RETRACK_USER_TAG: &str = "user";

/// Defines a tag that stores the notification flag.
pub const RETRACK_NOTIFICATIONS_TAG: &str = "notifications";

/// Defines a tag that stores the resource type.
pub const RETRACK_RESOURCE_TAG: &str = "resource";

/// Defines a tag that stores the resource ID.
pub const RETRACK_RESOURCE_ID_TAG: &str = "resource_id";

/// Prepares tags for the Retrack API by prefixing them with the `secutils`.
pub fn prepare_tags<Tag: AsRef<str>>(tags: &[Tag]) -> Vec<String> {
    once(RETRACK_APP_TAG.to_string())
        .chain(
            tags.iter()
                .map(|tag| format!("{RETRACK_APP_TAG}:{}", tag.as_ref())),
        )
        .collect()
}

/// Extracts value from the tag with the specified ID/prefix.
pub fn get_tag_value<Tag: AsRef<str>>(all_tags: &[Tag], identifier: &str) -> Option<String> {
    let prefix = format!("{RETRACK_APP_TAG}:{identifier}:");
    all_tags.iter().find_map(|tag| {
        let tag = tag.as_ref();
        if tag.starts_with(&prefix) {
            Some(tag[prefix.len()..].to_string())
        } else {
            None
        }
    })
}
