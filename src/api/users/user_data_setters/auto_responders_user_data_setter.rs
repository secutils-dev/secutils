use crate::{api::users::UserDataSetter, users::UserDataType, utils::AutoResponder};
use anyhow::{bail, Context};
use std::collections::BTreeMap;

pub struct AutoRespondersUserDataSetter;
impl AutoRespondersUserDataSetter {
    pub async fn upsert(
        data_setter: &UserDataSetter<'_>,
        serialized_data_value: Vec<u8>,
    ) -> anyhow::Result<()> {
        let from_value = serde_json::from_slice::<BTreeMap<String, Option<AutoResponder>>>(
            &serialized_data_value,
        )
        .with_context(|| "Cannot deserialize new responders data".to_string())?;

        let mut to_value: BTreeMap<_, _> = data_setter
            .get(UserDataType::AutoResponders)
            .await
            .with_context(|| "Cannot retrieve stored responders data".to_string())?
            .unwrap_or_default();

        for (alias, auto_responder) in from_value {
            if let Some(auto_responder) = auto_responder {
                if !auto_responder.is_valid() {
                    bail!("Responder is not valid: {:?}", auto_responder);
                }
                to_value.insert(alias, auto_responder);
            } else {
                to_value.remove(&alias);
            }
        }

        if to_value.is_empty() {
            data_setter.remove(UserDataType::AutoResponders).await
        } else {
            data_setter
                .upsert(UserDataType::AutoResponders, to_value)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::users::{AutoRespondersUserDataSetter, UserDataSetter},
        datastore::PrimaryDb,
        tests::MockUserBuilder,
        users::{User, UserDataType, UserId},
        utils::{tests::MockAutoResponder, AutoResponder, AutoResponderMethod},
    };
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    fn create_mock_user() -> User {
        MockUserBuilder::new(
            UserId::empty(),
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            OffsetDateTime::now_utc(),
        )
        .build()
    }

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn can_merge_auto_responders_data() -> anyhow::Result<()> {
        let mock_user = create_mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let user_data_setter = UserDataSetter::new(&mock_user.email, &mock_db);

        let auto_responder_one =
            MockAutoResponder::new("test-1-alias", AutoResponderMethod::Post, 300).build();
        let auto_responder_two =
            MockAutoResponder::new("test-2-alias", AutoResponderMethod::Post, 300)
                .set_requests_to_track(10)
                .set_body("body")
                .set_headers(vec![(
                    "Content-Type".to_string(),
                    "application/json".to_string(),
                )])
                .set_delay(1000)
                .build();
        let auto_responder_two_conflict =
            MockAutoResponder::new("test-2-alias", AutoResponderMethod::Get, 300)
                .set_body("body")
                .build();
        let auto_responder_three =
            MockAutoResponder::new("test-3-alias", AutoResponderMethod::Options, 403)
                .set_delay(2000)
                .build();

        // Fill empty data.
        let initial_responders = [
            (
                auto_responder_one.alias.to_string(),
                auto_responder_one.clone(),
            ),
            (
                auto_responder_two.alias.to_string(),
                auto_responder_two.clone(),
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        AutoRespondersUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&initial_responders)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::AutoResponders).await?,
            Some(initial_responders)
        );

        // Overwrite existing data and preserve non-conflicting existing data.
        let conflicting_responders = [(
            auto_responder_two_conflict.alias.to_string(),
            auto_responder_two_conflict.clone(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        AutoRespondersUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_responders)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::AutoResponders).await?,
            Some(
                [
                    (
                        auto_responder_one.alias.to_string(),
                        auto_responder_one.clone(),
                    ),
                    (
                        auto_responder_two_conflict.alias.to_string(),
                        auto_responder_two_conflict.clone(),
                    )
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete existing data.
        let conflicting_responders = [
            (auto_responder_two.alias.to_string(), None),
            (
                auto_responder_three.alias.to_string(),
                Some(auto_responder_three.clone()),
            ),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        AutoRespondersUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_responders)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::AutoResponders).await?,
            Some(
                [
                    (
                        auto_responder_one.alias.to_string(),
                        auto_responder_one.clone(),
                    ),
                    (
                        auto_responder_three.alias.to_string(),
                        auto_responder_three.clone(),
                    )
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete full slot.
        let conflicting_responders = [
            (auto_responder_one.alias.clone(), None),
            (auto_responder_three.alias, None),
        ]
        .into_iter()
        .collect::<BTreeMap<_, Option<AutoResponder>>>();
        AutoRespondersUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_responders)?,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, AutoResponder>>(UserDataType::AutoResponders)
                .await?,
            None
        );

        // Does nothing if there is nothing to delete.
        let conflicting_responders = [(auto_responder_one.alias, None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<AutoResponder>>>();
        AutoRespondersUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_responders)?,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, AutoResponder>>(UserDataType::AutoResponders)
                .await?,
            None
        );

        Ok(())
    }
}
