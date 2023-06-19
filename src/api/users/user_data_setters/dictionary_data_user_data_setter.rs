use crate::{
    api::users::UserDataSetter,
    users::{UserData, UserDataKey},
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;

pub struct DictionaryDataUserDataSetter;
impl DictionaryDataUserDataSetter {
    pub async fn upsert<R: DeserializeOwned + Serialize>(
        data_setter: &UserDataSetter<'_>,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<BTreeMap<String, Option<R>>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();

        let mut merged_user_data_value: BTreeMap<_, _> = data_setter
            .get(user_data_key)
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default();

        for (name, entry) in user_data.value {
            if let Some(entry) = entry {
                merged_user_data_value.insert(name, entry);
            } else {
                merged_user_data_value.remove(&name);
            }
        }

        if merged_user_data_value.is_empty() {
            data_setter.remove(user_data_key).await.map(|_| ())
        } else {
            data_setter
                .upsert(
                    user_data_key,
                    UserData::new(merged_user_data_value, user_data.timestamp),
                )
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::users::{DictionaryDataUserDataSetter, UserDataSetter},
        datastore::PrimaryDb,
        tests::{mock_db, mock_user},
        users::{PublicUserDataNamespace, User, UserData},
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn can_merge_data() -> anyhow::Result<()> {
        let mock_user = mock_user();
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
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                initial_items.clone(),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get(PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                initial_items,
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Overwrite existing data and preserve non-conflicting existing data.
        let conflicting_items = [("two".to_string(), Some(item_two_conflict.clone()))]
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                conflicting_items,
                OffsetDateTime::from_unix_timestamp(857720800)?,
            ),
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get(PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                [
                    ("one".to_string(), item_one.clone(),),
                    ("two".to_string(), item_two_conflict.clone(),)
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::from_unix_timestamp(857720800)?
            ))
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
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                conflicting_items,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get(PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                [
                    ("one".to_string(), item_one.clone(),),
                    ("three".to_string(), item_three.clone(),)
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Delete full slot.
        let conflicting_items = [("one".to_string(), None), ("three".to_string(), None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<serde_json::Value>>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                conflicting_items,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, serde_json::Value>>(PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        // Does nothing if there is nothing to delete.
        let conflicting_items = [("one".to_string(), None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<serde_json::Value>>>();
        DictionaryDataUserDataSetter::upsert::<serde_json::Value>(
            &user_data_setter,
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                conflicting_items,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, serde_json::Value>>(PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        Ok(())
    }
}
