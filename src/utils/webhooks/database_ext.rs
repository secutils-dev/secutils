mod raw_responder;
mod raw_responder_request;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::{Responder, ResponderMethod, ResponderRequest},
};
use anyhow::{anyhow, bail};
use raw_responder::RawResponder;
use raw_responder_request::RawResponderRequest;
use sqlx::{query, query_as, Pool, Sqlite};
use uuid::Uuid;

/// A database extension for the webhooks utility-related operations.
pub struct WebhooksDatabaseExt<'pool> {
    pool: &'pool Pool<Sqlite>,
}

impl<'pool> WebhooksDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Retrieves all responders.
    pub async fn get_responders(&self, user_id: UserId) -> anyhow::Result<Vec<Responder>> {
        let raw_responders = query_as!(
            RawResponder,
            r#"
SELECT id, name, path, method, settings, created_at
FROM user_data_webhooks_responders
WHERE user_id = ?1
ORDER BY created_at
                "#,
            *user_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut responders = vec![];
        for raw_responder in raw_responders {
            responders.push(Responder::try_from(raw_responder)?);
        }

        Ok(responders)
    }

    /// Retrieves responder for the specified user with the specified ID.
    pub async fn get_responder(
        &self,
        user_id: UserId,
        id: Uuid,
    ) -> anyhow::Result<Option<Responder>> {
        let id = id.as_ref();
        query_as!(
            RawResponder,
            r#"
        SELECT id, name, path, method, settings, created_at
        FROM user_data_webhooks_responders
        WHERE user_id = ?1 AND id = ?2
                        "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(Responder::try_from)
        .transpose()
    }

    /// Retrieves responder for the specified path and method.
    pub async fn find_responder(
        &self,
        user_id: UserId,
        path: &str,
        method: ResponderMethod,
    ) -> anyhow::Result<Option<Responder>> {
        let raw_method = RawResponder::get_raw_method(method)?;
        let raw_any_method = RawResponder::get_raw_method(ResponderMethod::Any)?;
        query_as!(
            RawResponder,
            r#"
        SELECT id, name, path, method, settings, created_at
        FROM user_data_webhooks_responders
        WHERE user_id = ?1 AND path = ?2 AND (method = ?3 OR method = ?4)
                        "#,
            *user_id,
            path,
            raw_method,
            raw_any_method
        )
        .fetch_optional(self.pool)
        .await?
        .map(Responder::try_from)
        .transpose()
    }

    /// Inserts responder.
    pub async fn insert_responder(
        &self,
        user_id: UserId,
        responder: &Responder,
    ) -> anyhow::Result<()> {
        let raw_responder = RawResponder::try_from(responder)?;
        let raw_any_method = RawResponder::get_raw_method(ResponderMethod::Any)?;
        // Construct a query that inserts a new responder only if there is no other existing
        // responder that already covers the same path and method.
        let result = query!(
                r#"
        WITH new_responder(user_id, id, name, path, method, settings, created_at) AS (
            VALUES ( ?1, ?2, ?3, ?4, ?5, ?6, ?7 )
        )
        INSERT INTO user_data_webhooks_responders (user_id, id, name, path, method, settings, created_at)
        SELECT * FROM new_responder
        WHERE NOT EXISTS(
            SELECT id FROM user_data_webhooks_responders 
            WHERE user_id = ?1 AND path = ?4 AND (method = ?8 OR ?5 = ?8)
        )
                "#,
                *user_id,
                raw_responder.id,
                raw_responder.name,
                raw_responder.path,
                raw_responder.method,
                raw_responder.settings,
                raw_responder.created_at,
                raw_any_method
            )
            .execute(self.pool)
            .await;

        match result {
            Ok(result) if result.rows_affected() > 0 => Ok(()),
            Ok(_) => {
                bail!(SecutilsError::client(format!(
                    "Responder with such path ('{}') and method ('{:?}') conflicts with another responder.",
                    responder.path, responder.method
                )))
            }
            Err(err) => {
                match err.as_database_error() {
                    Some(database_error) if database_error.is_unique_violation() => {
                        let error_message = if database_error.message().contains(".path") {
                            format!("Responder with such path ('{}') and method ('{:?}') already exists.", responder.path, responder.method)
                        } else {
                            format!(
                                "Responder with such name ('{}') already exists.",
                                responder.name
                            )
                        };
                        bail!(SecutilsError::client_with_root_cause(
                            anyhow!(err).context(error_message)
                        ))
                    }
                    _ => bail!(SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't create responder ('{}') due to unknown reason.",
                        responder.name
                    )))),
                }
            }
        }
    }

    /// Updates responder.
    pub async fn update_responder(
        &self,
        user_id: UserId,
        responder: &Responder,
    ) -> anyhow::Result<()> {
        let raw_responder = RawResponder::try_from(responder)?;
        let raw_any_method = RawResponder::get_raw_method(ResponderMethod::Any)?;
        // Construct a query that updates a new responder only if there is no other existing
        // responder that already covers the same path and method.
        let result = query!(
            r#"
    UPDATE user_data_webhooks_responders
    SET name = ?3, path = ?4, method = ?5, settings = ?6
    WHERE user_id = ?1 AND id = ?2 AND NOT EXISTS(
        SELECT id FROM user_data_webhooks_responders 
        WHERE user_id = ?1 AND id != ?2 AND path = ?4 AND (method = ?7 OR method = ?5 OR ?5 = ?7)
    )
            "#,
            *user_id,
            raw_responder.id,
            raw_responder.name,
            raw_responder.path,
            raw_responder.method,
            raw_responder.settings,
            raw_any_method
        )
        .execute(self.pool)
        .await;

        match result {
            Ok(result) if result.rows_affected() > 0 => Ok(()),
            Ok(_) => {
                bail!(SecutilsError::client(format!(
                    "Responder with such path ('{}') and method ('{:?}') doesn't exist or conflicts with another responder.",
                    responder.path, responder.method
                )))
            }
            Err(err) => {
                match err.as_database_error() {
                    Some(database_error) if database_error.is_unique_violation() => {
                        let error_message = if database_error.message().contains(".path") {
                            format!("Responder with such path ('{}') and method ('{:?}') already exists.", responder.path, responder.method)
                        } else {
                            format!(
                                "Responder with such name ('{}') already exists.",
                                responder.name
                            )
                        };
                        bail!(SecutilsError::client_with_root_cause(
                            anyhow!(err).context(error_message)
                        ))
                    }
                    _ => bail!(SecutilsError::from(anyhow!(err).context(format!(
                        "Couldn't update responder ('{}') due to unknown reason.",
                        responder.name
                    )))),
                }
            }
        }
    }

    /// Removes responder for the specified user with the specified ID.
    pub async fn remove_responder(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        let id = id.as_ref();
        query!(
            r#"
        DELETE FROM user_data_webhooks_responders
        WHERE user_id = ?1 AND id = ?2
                        "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all tracked requests for the specified responder.
    pub async fn get_responder_requests(
        &self,
        user_id: UserId,
        responder_id: Uuid,
    ) -> anyhow::Result<Vec<ResponderRequest<'static>>> {
        let raw_requests = query_as!(
            RawResponderRequest,
            r#"
    SELECT id, responder_id, data, created_at
    FROM user_data_webhooks_responders_history
    WHERE user_id = ?1 AND responder_id = ?2
    ORDER BY created_at
                    "#,
            *user_id,
            responder_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut requests = vec![];
        for raw_request in raw_requests {
            requests.push(ResponderRequest::try_from(raw_request)?);
        }

        Ok(requests)
    }

    /// Removes responder requests.
    pub async fn clear_responder_requests(
        &self,
        user_id: UserId,
        responder_id: Uuid,
    ) -> anyhow::Result<()> {
        let id = responder_id.as_ref();
        query!(
            r#"
        DELETE FROM user_data_webhooks_responders_history
        WHERE user_id = ?1 AND responder_id = ?2
                        "#,
            *user_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    // Inserts responder request.
    pub async fn insert_responder_request(
        &self,
        user_id: UserId,
        request: &ResponderRequest<'_>,
    ) -> anyhow::Result<()> {
        let raw_request = RawResponderRequest::try_from(request)?;
        let result = query!(
                r#"
        INSERT INTO user_data_webhooks_responders_history (user_id, id, responder_id, data, created_at)
        VALUES ( ?1, ?2, ?3, ?4, ?5 )
                "#,
                *user_id,
                raw_request.id,
                raw_request.responder_id,
                raw_request.data,
                raw_request.created_at
            )
            .execute(self.pool)
            .await;

        if let Err(err) = result {
            bail!(SecutilsError::from(anyhow!(err).context(format!(
                "Couldn't create responder request ('{}') due to unknown reason.",
                request.id
            ))));
        }

        Ok(())
    }

    /// Removes responder request.
    pub async fn remove_responder_request(
        &self,
        user_id: UserId,
        responder_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<()> {
        query!(
            r#"
        DELETE FROM user_data_webhooks_responders_history
        WHERE user_id = ?1 AND responder_id = ?2 AND id = ?3
                        "#,
            *user_id,
            responder_id,
            id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

impl Database {
    /// Returns a database extension for the webhooks utility-related operations.
    pub fn webhooks(&self) -> WebhooksDatabaseExt {
        WebhooksDatabaseExt::new(&self.pool)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error as SecutilsError,
        tests::{mock_db, mock_user, MockResponderBuilder},
        utils::{Responder, ResponderMethod, ResponderRequest},
    };
    use insta::assert_debug_snapshot;
    use std::{
        borrow::Cow,
        net::{IpAddr, Ipv4Addr},
    };
    use time::OffsetDateTime;
    use uuid::{uuid, Uuid};

    fn create_request(id: Uuid, responder_id: Uuid) -> anyhow::Result<ResponderRequest<'static>> {
        Ok(ResponderRequest {
            id,
            responder_id,
            client_address: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            method: Cow::Owned("post".to_string()),
            headers: Some(vec![(
                Cow::Owned("Content-Type".to_string()),
                Cow::Owned(vec![1, 2, 3]),
            )]),
            body: Some(Cow::Owned(vec![4, 5, 6])),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
    }

    #[tokio::test]
    async fn can_add_and_retrieve_responders() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut responders: Vec<Responder> = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        let responder = webhooks
            .get_responder(user.id, responders[0].id)
            .await?
            .unwrap();
        assert_eq!(responder, responders.remove(0));

        let responder = webhooks
            .get_responder(user.id, responders[0].id)
            .await?
            .unwrap();
        assert_eq!(responder, responders.remove(0));

        assert!(webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000005"))
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_responders_on_insert() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responder = MockResponderBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "/",
        )?
        .with_method(ResponderMethod::Post)
        .build();

        let webhooks = db.webhooks();
        webhooks.insert_responder(user.id, &responder).await?;

        // Same name.
        let insert_error = webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "/path",
                )?
                .with_method(ResponderMethod::Get)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            insert_error,
            @r###"
        Error {
            context: "Responder with such name (\'some-name\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_webhooks_responders.name, user_data_webhooks_responders.user_id",
                },
            ),
        }
        "###
        );

        // Same path and method.
        let insert_error = webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "/",
                )?
                .with_method(ResponderMethod::Post)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            insert_error,
            @r###"
        Error {
            context: "Responder with such path (\'/\') and method (\'Post\') already exists.",
            source: Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: user_data_webhooks_responders.path, user_data_webhooks_responders.method, user_data_webhooks_responders.user_id",
                },
            ),
        }
        "###
        );

        // Same path and ANY method.
        let insert_error = webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "/",
                )?
                .with_method(ResponderMethod::Any)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            insert_error,
            @r###""Responder with such path ('/') and method ('Any') conflicts with another responder.""###
        );

        webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "/path",
                )?
                .with_method(ResponderMethod::Any)
                .build(),
            )
            .await?;
        // Same path and conflicting method.
        let insert_error = webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000003"),
                    "some-name-3",
                    "/path",
                )?
                .with_method(ResponderMethod::Get)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            insert_error,
            @r###""Responder with such path ('/path') and method ('Get') conflicts with another responder.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_update_responder() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let webhooks = db.webhooks();
        webhooks
            .insert_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    "/",
                )?
                .build(),
            )
            .await?;

        webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    "/path",
                )?
                .with_body("some")
                .build(),
            )
            .await?;

        let responder = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(
            responder,
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name-2",
                "/path"
            )?
            .with_body("some")
            .build()
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_duplicated_responders_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let webhooks = db.webhooks();
        let responder_a = MockResponderBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000001"),
            "some-name",
            "/",
        )?
        .build();
        webhooks.insert_responder(user.id, &responder_a).await?;

        let responder_b = MockResponderBuilder::create(
            uuid!("00000000-0000-0000-0000-000000000002"),
            "some-name-2",
            "/path",
        )?
        .with_method(ResponderMethod::Post)
        .build();
        webhooks.insert_responder(user.id, &responder_b).await?;

        // Same name.
        let update_error = webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name",
                    "/path",
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###"
            Error {
                context: "Responder with such name (\'some-name\') already exists.",
                source: Database(
                    SqliteError {
                        code: 2067,
                        message: "UNIQUE constraint failed: user_data_webhooks_responders.name, user_data_webhooks_responders.user_id",
                    },
                ),
            }
            "###
        );

        // Same path and method.
        let update_error = webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "/",
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""Responder with such path ('/') and method ('Any') doesn't exist or conflicts with another responder.""###
        );

        // Same path and ANY method.
        let update_error = webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000002"),
                    "some-name-2",
                    "/",
                )?
                .with_method(ResponderMethod::Post)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""Responder with such path ('/') and method ('Post') doesn't exist or conflicts with another responder.""###
        );

        // Same path and method.
        let update_error = webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    "/path",
                )?
                .with_method(ResponderMethod::Post)
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""Responder with such path ('/path') and method ('Post') doesn't exist or conflicts with another responder.""###
        );

        // Same path and ANY method.
        let update_error = webhooks
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name",
                    "/path",
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""Responder with such path ('/path') and method ('Any') doesn't exist or conflicts with another responder.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn correctly_handles_non_existent_responders_on_update() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let update_error = db
            .webhooks()
            .update_responder(
                user.id,
                &MockResponderBuilder::create(
                    uuid!("00000000-0000-0000-0000-000000000001"),
                    "some-name-2",
                    "/",
                )?
                .build(),
            )
            .await
            .unwrap_err()
            .downcast::<SecutilsError>()
            .unwrap();
        assert_debug_snapshot!(
            update_error,
            @r###""Responder with such path ('/') and method ('Any') doesn't exist or conflicts with another responder.""###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_responders() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let mut responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        let responder_1 = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?
            .unwrap();
        assert_eq!(responder_1, responders.remove(0));

        let responder_2 = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(responder_2, responders[0].clone());

        webhooks
            .remove_responder(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;

        let responder = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(responder.is_none());

        let responder = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?
            .unwrap();
        assert_eq!(responder, responders.remove(0));

        webhooks
            .remove_responder(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;

        let responder = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000001"))
            .await?;
        assert!(responder.is_none());

        let responder = webhooks
            .get_responder(user.id, uuid!("00000000-0000-0000-0000-000000000002"))
            .await?;
        assert!(responder.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn can_retrieve_responders_for_path_and_method() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .with_method(ResponderMethod::Post)
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        for method in [
            ResponderMethod::Post,
            ResponderMethod::Get,
            ResponderMethod::Any,
            ResponderMethod::Delete,
            ResponderMethod::Connect,
            ResponderMethod::Head,
            ResponderMethod::Options,
            ResponderMethod::Patch,
            ResponderMethod::Put,
            ResponderMethod::Trace,
        ] {
            assert_eq!(
                webhooks.find_responder(user.id, "/", method).await?,
                Some(responders[0].clone())
            );

            if matches!(method, ResponderMethod::Post) {
                assert_eq!(
                    webhooks.find_responder(user.id, "/path", method).await?,
                    Some(responders[1].clone())
                );
            } else {
                assert_eq!(
                    webhooks.find_responder(user.id, "/path", method).await?,
                    None
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn can_retrieve_all_responders() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        assert_eq!(webhooks.get_responders(user.id).await?, responders);

        Ok(())
    }

    #[tokio::test]
    async fn can_add_and_retrieve_history_revisions() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        // No history yet.
        for responder in responders.iter() {
            assert!(webhooks
                .get_responder_requests(user.id, responder.id)
                .await?
                .is_empty());
        }

        let mut requests = vec![
            create_request(
                uuid!("00000000-0000-0000-0000-000000000001"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000002"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000003"),
                responders[1].id,
            )?,
        ];
        for request in requests.iter() {
            webhooks.insert_responder_request(user.id, request).await?;
        }

        let history = webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?;
        assert_eq!(history, vec![requests.remove(0), requests.remove(0)]);

        let history = webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?;
        assert_eq!(history, vec![requests.remove(0)]);

        assert!(webhooks
            .get_responder_requests(user.id, uuid!("00000000-0000-0000-0000-000000000004"))
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_remove_responder_requests() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        let requests = vec![
            create_request(
                uuid!("00000000-0000-0000-0000-000000000001"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000002"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000003"),
                responders[1].id,
            )?,
        ];
        for request in requests.iter() {
            webhooks.insert_responder_request(user.id, request).await?;
        }

        let history = webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?;
        assert_eq!(history, vec![requests[0].clone(), requests[1].clone()]);

        let history = webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?;
        assert_eq!(history, vec![requests[2].clone()]);

        // Remove one revision.
        webhooks
            .remove_responder_request(user.id, responders[0].id, requests[0].id)
            .await?;

        let history = webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?;
        assert_eq!(history, vec![requests[1].clone()]);

        let history = webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?;
        assert_eq!(history, vec![requests[2].clone()]);

        // Remove the rest of requests.
        webhooks
            .remove_responder_request(user.id, responders[0].id, requests[1].id)
            .await?;
        webhooks
            .remove_responder_request(user.id, responders[1].id, requests[2].id)
            .await?;

        assert!(webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?
            .is_empty());
        assert!(webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_clear_all_responder_requests_at_once() -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = mock_db().await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/",
            )?
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/path",
            )?
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        let requests = vec![
            create_request(
                uuid!("00000000-0000-0000-0000-000000000001"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000002"),
                responders[0].id,
            )?,
            create_request(
                uuid!("00000000-0000-0000-0000-000000000003"),
                responders[1].id,
            )?,
        ];
        for request in requests.iter() {
            webhooks.insert_responder_request(user.id, request).await?;
        }

        let history = webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?;
        assert_eq!(history, vec![requests[0].clone(), requests[1].clone()]);

        let history = webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?;
        assert_eq!(history, vec![requests[2].clone()]);

        // Clear all revisions.
        webhooks
            .clear_responder_requests(user.id, responders[0].id)
            .await?;
        webhooks
            .clear_responder_requests(user.id, responders[1].id)
            .await?;

        assert!(webhooks
            .get_responder_requests(user.id, responders[0].id)
            .await?
            .is_empty());
        assert!(webhooks
            .get_responder_requests(user.id, responders[1].id)
            .await?
            .is_empty());

        Ok(())
    }
}
