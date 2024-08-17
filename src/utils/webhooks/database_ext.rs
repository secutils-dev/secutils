mod raw_responder;
mod raw_responder_request;

use crate::{
    database::Database,
    error::Error as SecutilsError,
    users::UserId,
    utils::webhooks::{
        Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderRequest,
    },
};
use anyhow::{anyhow, bail};
use raw_responder::RawResponder;
use raw_responder_request::RawResponderRequest;
use sqlx::{query, query_as, Pool, Postgres};
use uuid::Uuid;

/// A database extension for the webhooks utility-related operations.
pub struct WebhooksDatabaseExt<'pool> {
    pool: &'pool Pool<Postgres>,
}

impl<'pool> WebhooksDatabaseExt<'pool> {
    pub fn new(pool: &'pool Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Retrieves all responders.
    pub async fn get_responders(&self, user_id: UserId) -> anyhow::Result<Vec<Responder>> {
        let raw_responders = query_as!(
            RawResponder,
            r#"
SELECT id, name, location, method, enabled, settings, created_at, updated_at
FROM user_data_webhooks_responders
WHERE user_id = $1
ORDER BY updated_at
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
        query_as!(
            RawResponder,
            r#"
        SELECT id, name, location, method, enabled, settings, created_at, updated_at
        FROM user_data_webhooks_responders
        WHERE user_id = $1 AND id = $2
                        "#,
            *user_id,
            id
        )
        .fetch_optional(self.pool)
        .await?
        .map(Responder::try_from)
        .transpose()
    }

    /// Retrieves responder for the specified subdomain prefix, path and method.
    pub async fn find_responder(
        &self,
        user_id: UserId,
        subdomain_prefix: Option<&str>,
        path: &str,
        method: ResponderMethod,
    ) -> anyhow::Result<Option<Responder>> {
        let raw_method = RawResponder::get_raw_method(method)?;
        let raw_any_method = RawResponder::get_raw_method(ResponderMethod::Any)?;

        let raw_location_exact = ResponderLocation {
            path_type: ResponderPathType::Exact,
            path: path.to_string(),
            subdomain_prefix: subdomain_prefix.map(|s| s.to_string()),
        }
        .to_string();
        let raw_location_prefix = ResponderLocation {
            path_type: ResponderPathType::Prefix,
            path: path.to_string(),
            subdomain_prefix: subdomain_prefix.map(|s| s.to_string()),
        }
        .to_string();

        // Find the most specific responder ("ORDER BY length(location) DESC") that matches the
        // location and method. The "ORDER BY location DESC" means that we prefer exact match to a
        // prefix match ("=" vs "^" in natural sort).
        query_as!(
            RawResponder,
            r#"
        SELECT id, name, location, method, enabled, settings, created_at, updated_at
        FROM user_data_webhooks_responders
        WHERE user_id = $1 AND (location = $2 OR starts_with($3, location COLLATE "und-x-icu")) AND (method = $4 OR method = $5)
        ORDER BY length(location) DESC, location DESC
        LIMIT 1
                        "#,
            *user_id,
            raw_location_exact,
            raw_location_prefix,
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
        let id = *user_id;
        let raw_any_method = RawResponder::get_raw_method(ResponderMethod::Any)?;
        // Construct a query that inserts a new responder only if there is no other existing
        // responder that already covers the same location and method.
        let result = query!(
                r#"
        WITH new_responder(user_id, id, name, location, method, enabled, settings, created_at, updated_at) AS (
            VALUES ( $1::uuid, $2::uuid, $3, $4, $5::bytea, $6::bool, $7::bytea, $8::timestamptz, $9::timestamptz )
        )
        INSERT INTO user_data_webhooks_responders (user_id, id, name, location, method, enabled, settings, created_at, updated_at)
        SELECT * FROM new_responder
        WHERE NOT EXISTS(
            SELECT id FROM user_data_webhooks_responders 
            WHERE user_id = $1 AND location = $4 AND (method = $10 OR $5 = $10)
        )
                "#,
                id,
                raw_responder.id,
                raw_responder.name,
                raw_responder.location,
                raw_responder.method,
                raw_responder.enabled,
                raw_responder.settings,
                raw_responder.created_at,
                raw_responder.updated_at,
                raw_any_method
            )
            .execute(self.pool)
            .await;

        match result {
            Ok(result) if result.rows_affected() > 0 => Ok(()),
            Ok(_) => {
                bail!(SecutilsError::client(format!(
                    "Responder with such location ('{:?}') and method ('{:?}') conflicts with another responder.",
                    &responder.location, responder.method
                )))
            }
            Err(err) => match err.as_database_error() {
                Some(database_error) if database_error.is_unique_violation() => {
                    let error_message = if database_error.message().contains(".location") {
                        format!("Responder with such location ('{:?}') and method ('{:?}') already exists.", &responder.location, responder.method)
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
            },
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
        // responder that already covers the same location and method.
        let result = query!(
            r#"
    UPDATE user_data_webhooks_responders
    SET name = $3, location = $4, method = $5, enabled = $6, settings = $7, updated_at = $8
    WHERE user_id = $1 AND id = $2 AND NOT EXISTS(
        SELECT id FROM user_data_webhooks_responders 
        WHERE user_id = $1 AND id != $2 AND location = $4 AND (method = $9 OR method = $5 OR $5 = $9)
    )
            "#,
            *user_id,
            raw_responder.id,
            raw_responder.name,
            raw_responder.location,
            raw_responder.method,
            raw_responder.enabled,
            raw_responder.settings,
            raw_responder.updated_at,
            raw_any_method
        )
            .execute(self.pool)
            .await;

        match result {
            Ok(result) if result.rows_affected() > 0 => Ok(()),
            Ok(_) => {
                bail!(SecutilsError::client(format!(
                    "Responder with such location ('{:?}') and method ('{:?}') doesn't exist or conflicts with another responder.",
                    &responder.location, responder.method
                )))
            }
            Err(err) => match err.as_database_error() {
                Some(database_error) if database_error.is_unique_violation() => {
                    let error_message = if database_error.message().contains(".location") {
                        format!("Responder with such location ('{:?}') and method ('{:?}') already exists.", &responder.location, responder.method)
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
            },
        }
    }

    /// Removes responder for the specified user with the specified ID.
    pub async fn remove_responder(&self, user_id: UserId, id: Uuid) -> anyhow::Result<()> {
        query!(
            r#"
        DELETE FROM user_data_webhooks_responders
        WHERE user_id = $1 AND id = $2
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
    WHERE user_id = $1 AND responder_id = $2
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
        query!(
            r#"
        DELETE FROM user_data_webhooks_responders_history
        WHERE user_id = $1 AND responder_id = $2
                        "#,
            *user_id,
            responder_id
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
        VALUES ( $1, $2, $3, $4, $5 )
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
        WHERE user_id = $1 AND responder_id = $2 AND id = $3
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
        database::Database,
        error::Error as SecutilsError,
        tests::{mock_user, to_database_error, MockResponderBuilder},
        utils::webhooks::{
            Responder, ResponderLocation, ResponderMethod, ResponderPathType, ResponderRequest,
        },
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::borrow::Cow;
    use time::OffsetDateTime;
    use uuid::{uuid, Uuid};

    fn create_request(id: Uuid, responder_id: Uuid) -> anyhow::Result<ResponderRequest<'static>> {
        Ok(ResponderRequest {
            id,
            responder_id,
            client_address: Some("127.0.0.1:8080".parse()?),
            method: Cow::Owned("post".to_string()),
            headers: Some(vec![(
                Cow::Owned("Content-Type".to_string()),
                Cow::Owned(vec![1, 2, 3]),
            )]),
            url: Cow::Borrowed("/some-path?query=value"),
            body: Some(Cow::Owned(vec![4, 5, 6])),
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
    }

    #[sqlx::test]
    async fn can_add_and_retrieve_responders(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn correctly_handles_duplicated_responders_on_insert(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            insert_error.root_cause.to_string(),
            @r###""Responder with such name ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(insert_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_webhooks_responders_name_user_id_key\"""###
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
            insert_error.root_cause.to_string(),
            @r###""Responder with such name ('some-name-2') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(insert_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_webhooks_responders_path_method_user_id_key\"""###
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
            @r###""Responder with such location ('/ (Exact)') and method ('Any') conflicts with another responder.""###
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
            @r###""Responder with such location ('/path (Exact)') and method ('Get') conflicts with another responder.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_responder(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn correctly_handles_duplicated_responders_on_update(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            update_error.root_cause.to_string(),
            @r###""Responder with such name ('some-name') already exists.""###
        );
        assert_debug_snapshot!(
            to_database_error(update_error.root_cause)?.message(),
            @r###""duplicate key value violates unique constraint \"user_data_webhooks_responders_name_user_id_key\"""###
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
            @r###""Responder with such location ('/ (Exact)') and method ('Any') doesn't exist or conflicts with another responder.""###
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
            @r###""Responder with such location ('/ (Exact)') and method ('Post') doesn't exist or conflicts with another responder.""###
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
            @r###""Responder with such location ('/path (Exact)') and method ('Post') doesn't exist or conflicts with another responder.""###
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
            @r###""Responder with such location ('/path (Exact)') and method ('Any') doesn't exist or conflicts with another responder.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn correctly_handles_non_existent_responders_on_update(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
            @r###""Responder with such location ('/ (Exact)') and method ('Any') doesn't exist or conflicts with another responder.""###
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_responders(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn can_retrieve_responders_for_path_and_method(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
                webhooks.find_responder(user.id, None, "/", method).await?,
                Some(responders[0].clone())
            );

            if matches!(method, ResponderMethod::Post) {
                assert_eq!(
                    webhooks
                        .find_responder(user.id, None, "/path", method)
                        .await?,
                    Some(responders[1].clone())
                );
            } else {
                assert_eq!(
                    webhooks
                        .find_responder(user.id, None, "/path", method)
                        .await?,
                    None
                );
            }
        }

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_responders_for_location(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/a/b",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/a/b".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-2",
                "/a/b/c",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/a/b/c".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000003"),
                "some-name-3",
                "/a",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/a".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000004"),
                "some-name-4",
                "/a/b/c/d",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/a/b/c/d".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/", ResponderMethod::Get)
                .await?,
            None
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a", ResponderMethod::Get)
                .await?,
            Some(responders[2].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b", ResponderMethod::Get)
                .await?,
            Some(responders[0].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c/d", ResponderMethod::Get)
                .await?,
            Some(responders[3].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c/d/e", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_catch_all_responder(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
        db.insert_user(&user).await?;

        let responders = vec![
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000001"),
                "some-name",
                "/a/b/c/d",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Exact,
                path: "/a/b/c/d".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
            MockResponderBuilder::create(
                uuid!("00000000-0000-0000-0000-000000000002"),
                "some-name-catch-all",
                "/",
            )?
            .with_location(ResponderLocation {
                path_type: ResponderPathType::Prefix,
                path: "/".to_string(),
                subdomain_prefix: Some("sub".to_string()),
            })
            .build(),
        ];

        let webhooks = db.webhooks();
        for responder in responders.iter() {
            webhooks.insert_responder(user.id, responder).await?;
        }

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c/d", ResponderMethod::Get)
                .await?,
            Some(responders[0].clone())
        );

        assert_eq!(
            webhooks
                .find_responder(user.id, Some("sub"), "/a/b/c/d/e", ResponderMethod::Get)
                .await?,
            Some(responders[1].clone())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_retrieve_all_responders(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn can_add_and_retrieve_history_revisions(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn can_remove_responder_requests(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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

    #[sqlx::test]
    async fn can_clear_all_responder_requests_at_once(pool: PgPool) -> anyhow::Result<()> {
        let user = mock_user()?;
        let db = Database::create(pool).await?;
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
