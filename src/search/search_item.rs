use crate::users::UserId;
use serde::Serialize;
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};
use time::OffsetDateTime;

/// Represents a search hit.
#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
pub struct SearchItem {
    #[serde(skip_serializing)]
    pub id: u64,
    #[serde(rename = "l")]
    pub label: String,
    #[serde(rename = "c")]
    pub category: String,
    #[serde(skip_serializing)]
    pub keywords: Option<String>,
    #[serde(rename = "s", skip_serializing_if = "Option::is_none")]
    pub sub_category: Option<String>,
    #[serde(skip_serializing)]
    pub user_id: Option<UserId>,
    #[serde(rename = "m", skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(rename = "t", with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
}

impl SearchItem {
    pub fn create_id(
        label: &str,
        category: &str,
        sub_category: Option<&str>,
        user_id: Option<UserId>,
    ) -> u64 {
        let mut s = DefaultHasher::new();
        label.hash(&mut s);
        category.hash(&mut s);

        if let Some(sub_category) = sub_category {
            sub_category.hash(&mut s);
        }

        if let Some(user_id) = user_id {
            user_id.hash(&mut s);
        }
        s.finish()
    }
}

impl AsRef<SearchItem> for SearchItem {
    fn as_ref(&self) -> &SearchItem {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::search::SearchItem;
    use insta::{assert_debug_snapshot, assert_json_snapshot};
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let item_without_optional = SearchItem {
            id: 1,
            label: "some-label".to_string(),
            category: "some-category".to_string(),
            keywords: None,
            sub_category: None,
            user_id: None,
            meta: None,
            // January 1, 2010 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
        };
        assert_json_snapshot!(item_without_optional, @r###"
        {
          "l": "some-label",
          "c": "some-category",
          "t": 1262340000
        }
        "###);

        let item_with_optional = SearchItem {
            id: 1,
            label: "some-label".to_string(),
            keywords: Some("some keywords".to_string()),
            category: "some-category".to_string(),
            sub_category: Some("some-sub-category".to_string()),
            user_id: Some(uuid!("00000000-0000-0000-0000-000000000002").into()),
            meta: Some(
                [("one".to_string(), "two".to_string())]
                    .into_iter()
                    .collect(),
            ),
            // January 1, 2010 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
        };
        assert_json_snapshot!(item_with_optional, @r###"
        {
          "l": "some-label",
          "c": "some-category",
          "s": "some-sub-category",
          "m": {
            "one": "two"
          },
          "t": 1262340000
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_create_id() -> anyhow::Result<()> {
        assert_debug_snapshot!(SearchItem::create_id("some-label", "some-category", None, None), @"9401142304413078507");
        assert_debug_snapshot!(SearchItem::create_id("some-label", "some-category", Some("some-sub-category"), None), @"1596497830688235325");
        assert_debug_snapshot!(SearchItem::create_id("some-label", "some-category", None, Some(uuid!("00000000-0000-0000-0000-000000000001").into())), @"12198784850714205533");
        assert_debug_snapshot!(SearchItem::create_id("some-label", "some-category", Some("some-sub-category"), Some(uuid!("00000000-0000-0000-0000-000000000001").into())), @"7522693773249480917");

        Ok(())
    }
}
