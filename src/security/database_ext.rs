mod raw_user_webauthn_session;

use crate::{database::Database, security::WebAuthnSession};
use anyhow::Context;
use raw_user_webauthn_session::RawUserWebAuthnSession;
use sqlx::{query, query_as};
use time::OffsetDateTime;

/// Extends primary database with the authentication-related methods.
impl Database {
    /// Retrieves user's WebAuthn session from the `UserWebAuthnSessions` table using user email.
    pub async fn get_user_webauthn_session_by_email<E: AsRef<str>>(
        &self,
        email: E,
    ) -> anyhow::Result<Option<WebAuthnSession>> {
        let email = email.as_ref();
        query_as!(
            RawUserWebAuthnSession,
            r#"
SELECT email, session_value, timestamp
FROM user_webauthn_sessions
WHERE email = $1
                "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(WebAuthnSession::try_from)
        .transpose()
    }

    /// Sets user's WebAuthn session in the `UserWebAuthnSessions` table.
    pub async fn upsert_user_webauthn_session(
        &self,
        session: &WebAuthnSession,
    ) -> anyhow::Result<()> {
        let raw_session_value = serde_json::ser::to_vec(&session.value).with_context(|| {
            format!(
                "Failed to serialize user WebAuthn session ({}).",
                session.email
            )
        })?;

        query!(
            r#"
INSERT INTO user_webauthn_sessions (email, session_value, timestamp)
VALUES ($1, $2, $3)
ON CONFLICT(email) DO UPDATE SET session_value=excluded.session_value, timestamp=excluded.timestamp
        "#,
            session.email,
            raw_session_value,
            session.timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user's WebAuthn session from the `UserWebAuthnSessions` table using user email.
    pub async fn remove_user_webauthn_session_by_email(&self, email: &str) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_webauthn_sessions
WHERE email = $1
            "#,
            email
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes WebAuthn sessions that are older than specified timestamp.
    pub async fn remove_user_webauthn_sessions(&self, since: OffsetDateTime) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_webauthn_sessions
WHERE timestamp <= $1
            "#,
            since
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        database::Database,
        security::{WebAuthnSession, WebAuthnSessionValue},
        tests::webauthn::{SERIALIZED_AUTHENTICATION_STATE, SERIALIZED_REGISTRATION_STATE},
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;

    #[sqlx::test]
    async fn can_add_and_retrieve_webauthn_sessions(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        assert!(db
            .get_user_webauthn_session_by_email("some-id")
            .await?
            .is_none());

        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        assert_debug_snapshot!(db.get_user_webauthn_session_by_email("dev@secutils.dev").await?, @r###"
        Some(
            WebAuthnSession {
                email: "dev@secutils.dev",
                value: RegistrationState(
                    PasskeyRegistration {
                        rs: RegistrationState {
                            policy: Preferred,
                            exclude_credentials: [],
                            challenge: Base64UrlSafeData(
                                [
                                    223,
                                    161,
                                    90,
                                    219,
                                    14,
                                    74,
                                    186,
                                    255,
                                    52,
                                    157,
                                    60,
                                    210,
                                    28,
                                    75,
                                    219,
                                    3,
                                    154,
                                    213,
                                    19,
                                    100,
                                    38,
                                    255,
                                    29,
                                    40,
                                    142,
                                    55,
                                    15,
                                    45,
                                    135,
                                    129,
                                    245,
                                    18,
                                ],
                            ),
                            credential_algorithms: [
                                ES256,
                                RS256,
                            ],
                            require_resident_key: false,
                            authenticator_attachment: None,
                            extensions: RequestRegistrationExtensions {
                                cred_protect: None,
                                uvm: Some(
                                    true,
                                ),
                                cred_props: Some(
                                    true,
                                ),
                                min_pin_length: None,
                                hmac_create_secret: None,
                            },
                            experimental_allow_passkeys: true,
                        },
                    },
                ),
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_webauthn_session_by_email("prod@secutils.dev").await?, @r###"
        Some(
            WebAuthnSession {
                email: "prod@secutils.dev",
                value: RegistrationState(
                    PasskeyRegistration {
                        rs: RegistrationState {
                            policy: Preferred,
                            exclude_credentials: [],
                            challenge: Base64UrlSafeData(
                                [
                                    223,
                                    161,
                                    90,
                                    219,
                                    14,
                                    74,
                                    186,
                                    255,
                                    52,
                                    157,
                                    60,
                                    210,
                                    28,
                                    75,
                                    219,
                                    3,
                                    154,
                                    213,
                                    19,
                                    100,
                                    38,
                                    255,
                                    29,
                                    40,
                                    142,
                                    55,
                                    15,
                                    45,
                                    135,
                                    129,
                                    245,
                                    18,
                                ],
                            ),
                            credential_algorithms: [
                                ES256,
                                RS256,
                            ],
                            require_resident_key: false,
                            authenticator_attachment: None,
                            extensions: RequestRegistrationExtensions {
                                cred_protect: None,
                                uvm: Some(
                                    true,
                                ),
                                cred_props: Some(
                                    true,
                                ),
                                min_pin_length: None,
                                hmac_create_secret: None,
                            },
                            experimental_allow_passkeys: true,
                        },
                    },
                ),
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert!(db
            .get_user_by_email("unknown@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn ignores_email_case_for_webauthn_sessions(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
            ))?,
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
        .await?;

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("DeV@secUtils.dEv")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_update_webauthn_sessions(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
            ))?,
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
        })
        .await?;

        let session = db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .unwrap();
        assert_eq!(session.email, "dev@secutils.dev");
        assert_eq!(
            session.timestamp,
            OffsetDateTime::from_unix_timestamp(946720800)?
        );
        assert!(matches!(
            session.value,
            WebAuthnSessionValue::RegistrationState(_)
        ));

        db.upsert_user_webauthn_session(&WebAuthnSession {
            email: "dev@secutils.dev".to_string(),
            value: serde_json::from_str(&format!(
                "{{\"AuthenticationState\":{SERIALIZED_AUTHENTICATION_STATE}}}"
            ))?,
            // January 1, 2010 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
        })
        .await?;

        let session = db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .unwrap();
        assert_eq!(session.email, "dev@secutils.dev");
        assert_eq!(
            session.timestamp,
            OffsetDateTime::from_unix_timestamp(1262340000)?
        );
        assert!(matches!(
            session.value,
            WebAuthnSessionValue::AuthenticationState(_)
        ));

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_webauthn_session(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        db.remove_user_webauthn_session_by_email("dev@secutils.dev")
            .await?;
        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_session_by_email("PROD@secutils.dev")
            .await?;
        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn can_remove_old_webauthn_session(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let sessions = vec![
            WebAuthnSession {
                email: "dev@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2000 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            },
            WebAuthnSession {
                email: "prod@secutils.dev".to_string(),
                value: serde_json::from_str(&format!(
                    "{{\"RegistrationState\":{SERIALIZED_REGISTRATION_STATE}}}"
                ))?,
                // January 1, 2010 11:00:00
                timestamp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            },
        ];
        for session in sessions {
            db.upsert_user_webauthn_session(&session).await?;
        }

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(946720800)?.sub(Duration::from_secs(60)),
        )
        .await?;

        assert_eq!(
            db.get_user_webauthn_session_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .email,
            "dev@secutils.dev"
        );
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(946720800)?.add(Duration::from_secs(60)),
        )
        .await?;

        assert!(db
            .get_user_webauthn_session_by_email("dev@secutils.dev")
            .await?
            .is_none());
        assert_eq!(
            db.get_user_webauthn_session_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .email,
            "prod@secutils.dev"
        );

        db.remove_user_webauthn_sessions(
            OffsetDateTime::from_unix_timestamp(1262340000)?.add(Duration::from_secs(60)),
        )
        .await?;

        assert!(db
            .get_user_webauthn_session_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }
}
