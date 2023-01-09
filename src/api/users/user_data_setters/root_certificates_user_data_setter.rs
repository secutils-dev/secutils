use crate::{api::users::UserDataSetter, users::UserDataType, utils::RootCertificate};
use anyhow::Context;
use std::collections::BTreeMap;

pub struct RootCertificatesUserDataSetter;
impl RootCertificatesUserDataSetter {
    pub async fn upsert(
        data_setter: &UserDataSetter<'_>,
        serialized_data_value: Vec<u8>,
    ) -> anyhow::Result<()> {
        let from_value = serde_json::from_slice::<BTreeMap<String, Option<RootCertificate>>>(
            &serialized_data_value,
        )
        .with_context(|| "Cannot deserialize new root certificate data".to_string())?;

        let mut to_value: BTreeMap<_, _> = data_setter
            .get(UserDataType::RootCertificates)
            .await
            .with_context(|| "Cannot retrieve stored root certificates data".to_string())?
            .unwrap_or_default();

        for (alias, entry) in from_value {
            if let Some(entry) = entry {
                to_value.insert(alias, entry);
            } else {
                to_value.remove(&alias);
            }
        }

        if to_value.is_empty() {
            data_setter.remove(UserDataType::RootCertificates).await
        } else {
            data_setter
                .upsert(UserDataType::RootCertificates, to_value)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::users::{RootCertificatesUserDataSetter, UserDataSetter},
        datastore::PrimaryDb,
        tests::MockUserBuilder,
        users::{User, UserDataType, UserId},
        utils::{
            tests::MockRootCertificate, PublicKeyAlgorithm, RootCertificate, SignatureAlgorithm,
        },
    };
    use std::collections::BTreeMap;
    use time::OffsetDateTime;

    fn create_mock_user() -> User {
        MockUserBuilder::new(
            UserId(1),
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
    async fn can_merge_data() -> anyhow::Result<()> {
        let mock_user = create_mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let user_data_setter = UserDataSetter::new(mock_user.id, &mock_db);

        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        let item_one = MockRootCertificate::new(
            "test-1-alias",
            PublicKeyAlgorithm::Rsa,
            SignatureAlgorithm::Sha256,
            not_valid_before,
            not_valid_after,
            1,
        )
        .build();
        let item_two = MockRootCertificate::new(
            "test-2-alias",
            PublicKeyAlgorithm::Ed25519,
            SignatureAlgorithm::Ed25519,
            not_valid_before,
            not_valid_after,
            3,
        )
        .set_common_name("CA Issuer")
        .set_country("US")
        .set_state_or_province("California")
        .set_locality("San Francisco")
        .set_organization("CA Issuer, Inc")
        .set_organization_unit("CA Org Unit")
        .build();
        let item_two_conflict = MockRootCertificate::new(
            "test-2-alias",
            PublicKeyAlgorithm::Rsa,
            SignatureAlgorithm::Sha384,
            not_valid_before,
            not_valid_after.replace_year(2050)?,
            2,
        )
        .set_country("DE")
        .build();
        let item_three = MockRootCertificate::new(
            "test-3-alias",
            PublicKeyAlgorithm::Dsa,
            SignatureAlgorithm::Md5,
            not_valid_before,
            not_valid_after,
            1,
        )
        .set_common_name("Old CA Issuer")
        .build();

        // Fill empty data.
        let initial_items = [
            (item_one.alias.to_string(), item_one.clone()),
            (item_two.alias.to_string(), item_two.clone()),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        RootCertificatesUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&initial_items)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::RootCertificates).await?,
            Some(initial_items)
        );

        // Overwrite existing data and preserve non-conflicting existing data.
        let conflicting_items = [(
            item_two_conflict.alias.to_string(),
            item_two_conflict.clone(),
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        RootCertificatesUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_items)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::RootCertificates).await?,
            Some(
                [
                    (item_one.alias.to_string(), item_one.clone(),),
                    (
                        item_two_conflict.alias.to_string(),
                        item_two_conflict.clone(),
                    )
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete existing data.
        let conflicting_items = [
            (item_two.alias.to_string(), None),
            (item_three.alias.to_string(), Some(item_three.clone())),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        RootCertificatesUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_items)?,
        )
        .await?;
        assert_eq!(
            user_data_setter.get(UserDataType::RootCertificates).await?,
            Some(
                [
                    (item_one.alias.to_string(), item_one.clone(),),
                    (item_three.alias.to_string(), item_three.clone(),)
                ]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
            )
        );

        // Delete full slot.
        let conflicting_items = [(item_one.alias.clone(), None), (item_three.alias, None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<RootCertificate>>>();
        RootCertificatesUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_items)?,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, RootCertificate>>(UserDataType::RootCertificates)
                .await?,
            None
        );

        // Does nothing if there is nothing to delete.
        let conflicting_items = [(item_one.alias, None)]
            .into_iter()
            .collect::<BTreeMap<_, Option<RootCertificate>>>();
        RootCertificatesUserDataSetter::upsert(
            &user_data_setter,
            serde_json::ser::to_vec(&conflicting_items)?,
        )
        .await?;
        assert_eq!(
            user_data_setter
                .get::<BTreeMap<String, RootCertificate>>(UserDataType::AutoResponders)
                .await?,
            None
        );

        Ok(())
    }
}
