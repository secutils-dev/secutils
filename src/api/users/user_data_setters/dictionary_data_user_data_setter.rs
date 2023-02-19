use crate::api::users::UserDataSetter;
use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;

pub struct DictionaryDataUserDataSetter;
impl DictionaryDataUserDataSetter {
    pub async fn upsert<R: DeserializeOwned + Serialize>(
        data_setter: &UserDataSetter<'_>,
        data_key: &str,
        data_value: BTreeMap<String, Option<R>>,
    ) -> anyhow::Result<()> {
        let mut existing_value: BTreeMap<_, _> = data_setter
            .get(data_key)
            .await
            .with_context(|| format!("Cannot retrieve stored '{data_key}' data"))?
            .unwrap_or_default();

        for (name, entry) in data_value {
            if let Some(entry) = entry {
                existing_value.insert(name, entry);
            } else {
                existing_value.remove(&name);
            }
        }

        if existing_value.is_empty() {
            data_setter.remove(data_key).await
        } else {
            data_setter.upsert(data_key, existing_value).await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::users::{DictionaryDataUserDataSetter, UserDataSetter},
        authentication::StoredCredentials,
        datastore::PrimaryDb,
        tests::MockUserBuilder,
        users::{User, UserId},
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    fn create_mock_user() -> User {
        MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials::try_from_password("pass").unwrap(),
            OffsetDateTime::now_utc(),
        )
        .build()
    }

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn can_merge_data() -> anyhow::Result<()> {
        let mock_user = create_mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let user_data_setter = UserDataSetter::new(mock_user.id, &mock_db);

        let item_one = json!({ "name": "one" });
        let item_two = json!({ "name": "two" });
        let item_two_conflict = json!({ "name": "two-conflict" });
        let item_three = json!({ "name": "three" });

        // Fill empty data.
        let initial_items = [
            ("one".to_string(), Some(item_one.clone())),
            ("two".to_string(), Some(item_two.clone())),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            "data-key",
            initial_items.clone(),
        )
        .await?;
        assert_eq!(user_data_setter.get("data-key").await?, Some(initial_items));

        // Overwrite existing data and preserve non-conflicting existing data.
        let conflicting_items = [("two".to_string(), Some(item_two_conflict.clone()))]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            "data-key",
            conflicting_items,
        )
        .await?;
        assert_eq!(
            user_data_setter.get("data-key").await?,
            Some(
                [
                    ("one".to_string(), item_one.clone(),),
                    ("two".to_string(), item_two_conflict.clone(),)
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete existing data.
        let conflicting_items = [
            ("two".to_string(), None),
            ("three".to_string(), Some(item_three.clone())),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            "data-key",
            conflicting_items,
        )
        .await?;
        assert_eq!(
            user_data_setter.get("data-key").await?,
            Some(
                [
                    ("one".to_string(), item_one.clone(),),
                    ("three".to_string(), item_three.clone(),)
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete full slot.
        let conflicting_items = [("one".to_string(), None), ("three".to_string(), None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<serde_json::Value>>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            "data-key",
            conflicting_items,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, serde_json::Value>>("data-key")
                .await?,
            None
        );

        // Does nothing if there is nothing to delete.
        let conflicting_items = [("one".to_string(), None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<serde_json::Value>>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            "data-key",
            conflicting_items,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, serde_json::Value>>("data-key")
                .await?,
            None
        );

        Ok(())
    }
}
