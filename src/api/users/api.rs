use crate::{
    api::users::DictionaryDataUserDataSetter,
    database::Database,
    users::{
        BuiltinUser, PublicUserDataNamespace, User, UserData, UserDataKey, UserDataNamespace,
        UserId, UserSettingsSetter,
    },
    utils::{AutoResponder, ContentSecurityPolicy, SelfSignedCertificate},
};
use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use std::{borrow::Cow, collections::BTreeMap};
use time::OffsetDateTime;

pub struct UsersApi<'a> {
    db: Cow<'a, Database>,
}

impl<'a> UsersApi<'a> {
    /// Creates Users API.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db: Cow::Borrowed(db),
        }
    }

    /// Retrieves the user using the specified email.
    pub async fn get_by_email<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.db.get_user_by_email(user_email).await
    }

    /// Retrieves the user using the specified handle.
    pub async fn get_by_handle<E: AsRef<str>>(
        &self,
        user_handle: E,
    ) -> anyhow::Result<Option<User>> {
        self.db.get_user_by_handle(user_handle).await
    }

    /// Inserts or updates user in the `Users` store.
    pub async fn upsert<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        self.db.upsert_user(user).await
    }

    /// Inserts or updates user in the `Users` store using `BuiltinUser`.
    pub async fn upsert_builtin(&self, builtin_user: BuiltinUser) -> anyhow::Result<UserId> {
        let user = match self.db.get_user_by_email(&builtin_user.email).await? {
            Some(user) => User {
                id: user.id,
                email: user.email,
                handle: builtin_user.handle,
                created: user.created,
                credentials: builtin_user.credentials,
                roles: builtin_user.roles,
                activated: true,
            },
            None => User {
                id: UserId::empty(),
                email: builtin_user.email,
                handle: builtin_user.handle,
                credentials: builtin_user.credentials,
                created: OffsetDateTime::now_utc(),
                roles: builtin_user.roles,
                activated: true,
            },
        };

        self.upsert(&user).await
    }

    /// Removes the user with the specified email.
    pub async fn remove_by_email<E: AsRef<str>>(
        &self,
        user_email: E,
    ) -> anyhow::Result<Option<User>> {
        self.db.remove_user_by_email(user_email).await
    }

    /// Retrieves data with the specified key for the user with the specified id.
    pub async fn get_data<R: DeserializeOwned>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        self.db.get_user_data(user_id, user_data_key).await
    }

    /// Stores user data under the specified key.
    pub async fn set_data(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        match user_data_key.namespace {
            UserDataNamespace::Public(namespace) => match namespace {
                PublicUserDataNamespace::AutoResponders => {
                    self.set_auto_responders_data(user_data).await
                }
                PublicUserDataNamespace::ContentSecurityPolicies => {
                    self.set_content_security_policies_data(user_data).await
                }
                PublicUserDataNamespace::SelfSignedCertificates => {
                    self.set_self_signed_certificates_data(user_data).await
                }
                PublicUserDataNamespace::UserSettings => {
                    self.set_user_settings_data(user_data).await
                }
                namespace => {
                    bail!("Namespace is not supported: {}.", namespace.as_ref())
                }
            },
            UserDataNamespace::Internal(_) => {
                self.db.upsert_user_data(user_data_key, user_data).await
            }
        }
    }

    async fn set_auto_responders_data(
        &self,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let auto_responders = serde_json::from_slice::<BTreeMap<String, Option<AutoResponder>>>(
            &serialized_user_data.value,
        )
        .with_context(|| "Cannot deserialize new autoresponders data".to_string())?;

        for (auto_responder_name, auto_responder) in auto_responders.iter() {
            match auto_responder {
                Some(auto_responder) if !auto_responder.is_valid() => {
                    bail!("Responder `{auto_responder_name}` is not valid: {auto_responder:?}");
                }
                Some(auto_responder) => {
                    log::debug!("Upserting `{auto_responder_name}` responder: {auto_responder:?}");
                }
                None => {
                    log::debug!("Removing `{auto_responder_name}` responder and its requests.");
                    self.db
                        .remove_user_data(
                            serialized_user_data.user_id,
                            (
                                PublicUserDataNamespace::AutoResponders,
                                auto_responder_name.as_str(),
                            ),
                        )
                        .await?;
                }
            }
        }

        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                serialized_user_data.user_id,
                auto_responders,
                serialized_user_data.timestamp,
            ),
        )
        .await
    }

    async fn set_user_settings_data(
        &self,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        let user_settings =
            serde_json::from_slice::<UserSettingsSetter>(&serialized_user_data.value)
                .with_context(|| "Cannot deserialize new user settings data".to_string())?;
        if !user_settings.is_valid() {
            bail!("User settings are not valid: {:?}", user_settings);
        }
        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::UserSettings,
            UserData::new(
                serialized_user_data.user_id,
                user_settings.into_inner(),
                serialized_user_data.timestamp,
            ),
        )
        .await
    }

    async fn set_content_security_policies_data(
        &self,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::ContentSecurityPolicies,
            UserData::new(
                serialized_user_data.user_id,
                serde_json::from_slice::<BTreeMap<String, Option<ContentSecurityPolicy>>>(
                    &serialized_user_data.value,
                )
                .with_context(|| {
                    "Cannot deserialize new content security policies data".to_string()
                })?,
                serialized_user_data.timestamp,
            ),
        )
        .await
    }

    async fn set_self_signed_certificates_data(
        &self,
        serialized_user_data: UserData<Vec<u8>>,
    ) -> anyhow::Result<()> {
        DictionaryDataUserDataSetter::upsert(
            &self.db,
            PublicUserDataNamespace::SelfSignedCertificates,
            UserData::new(
                serialized_user_data.user_id,
                serde_json::from_slice::<BTreeMap<String, Option<SelfSignedCertificate>>>(
                    &serialized_user_data.value,
                )
                .with_context(|| {
                    "Cannot deserialize new self-signed certificates data".to_string()
                })?,
                serialized_user_data.timestamp,
            ),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::UsersApi,
        database::Database,
        tests::{mock_db, mock_user},
        users::{PublicUserDataNamespace, User, UserData},
        utils::{AutoResponder, AutoResponderMethod, AutoResponderRequest},
    };
    use std::{borrow::Cow, collections::BTreeMap};
    use time::OffsetDateTime;

    async fn initialize_mock_db(user: &User) -> anyhow::Result<Database> {
        let db = mock_db().await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn can_update_auto_responders() -> anyhow::Result<()> {
        let mock_user = mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = UsersApi::new(&mock_db);

        let auto_responder_one = AutoResponder {
            name: "name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        let auto_responder_two = AutoResponder {
            name: "name-two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        let auto_responder_two_new = AutoResponder {
            name: "name-two".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: 10,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };

        // Insert auto responders data.
        api.set_data(
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                mock_user.id,
                serde_json::to_vec(
                    &[
                        (&auto_responder_one.name, auto_responder_one.clone()),
                        (&auto_responder_two.name, auto_responder_two.clone()),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                )?,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;

        let request_one = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        let request_two = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.2".parse()?),
            method: Cow::Borrowed("POST"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };

        // Insert auto responder requests.
        mock_db
            .upsert_user_data(
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder_one.name.as_str(),
                ),
                UserData::new_with_key(
                    mock_user.id,
                    &auto_responder_one.name,
                    vec![request_one.clone()],
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;
        mock_db
            .upsert_user_data(
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder_two.name.as_str(),
                ),
                UserData::new_with_key(
                    mock_user.id,
                    &auto_responder_two.name,
                    vec![request_two.clone()],
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;

        // Verify that requests were inserted.
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_one.name.as_str(),
                    )
                )
                .await?,
            Some(UserData::new_with_key(
                mock_user.id,
                &auto_responder_one.name,
                vec![request_one],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_two.name.as_str(),
                    ),
                )
                .await?,
            Some(UserData::new_with_key(
                mock_user.id,
                &auto_responder_two.name,
                vec![request_two.clone()],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Remove one auto responder and update another.
        api.set_data(
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                mock_user.id,
                serde_json::to_vec(
                    &[
                        (&auto_responder_one.name, None),
                        (
                            &auto_responder_two.name,
                            Some(auto_responder_two_new.clone()),
                        ),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                )?,
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        )
        .await?;

        // Verify that auto responders were correctly updated.
        assert_eq!(
            api.get_data(mock_user.id, PublicUserDataNamespace::AutoResponders)
                .await?,
            Some(UserData::new(
                mock_user.id,
                [(auto_responder_two.name, auto_responder_two_new.clone())]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Verify that requests were updated.
        assert_eq!(
            mock_db
                .get_user_data::<Vec<AutoResponderRequest>>(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_one.name.as_str(),
                    ),
                )
                .await?,
            None
        );
        assert_eq!(
            mock_db
                .get_user_data(
                    mock_user.id,
                    (
                        PublicUserDataNamespace::AutoResponders,
                        auto_responder_two_new.name.as_str(),
                    ),
                )
                .await?,
            Some(UserData::new_with_key(
                mock_user.id,
                &auto_responder_two_new.name,
                vec![request_two],
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        Ok(())
    }
}
