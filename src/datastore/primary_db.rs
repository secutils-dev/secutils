mod raw_scheduler_job_and_next_tick;
mod raw_scheduler_job_stored_data;
mod raw_scheduler_notification;
mod raw_user;
mod raw_user_data;
mod raw_user_to_upsert;
mod raw_user_webauthn_session;
mod raw_util;

use crate::{
    authentication::WebAuthnSession,
    users::{User, UserData, UserDataKey, UserId},
    utils::Util,
};
use anyhow::{bail, Context};
use raw_scheduler_job_and_next_tick::RawSchedulerJobAndNextTick;
use raw_scheduler_job_stored_data::RawSchedulerJobStoredData;
use raw_scheduler_notification::RawSchedulerNotification;
use raw_user::RawUser;
use raw_user_data::RawUserData;
use raw_user_to_upsert::RawUserToUpsert;
use raw_user_webauthn_session::RawUserWebAuthnSession;
use raw_util::RawUtil;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{query, query_as, query_scalar, sqlite::SqlitePool, Pool, QueryBuilder, Sqlite};
use std::{collections::HashMap, time::Duration};
use time::OffsetDateTime;
use tokio_cron_scheduler::{
    JobAndNextTick, JobId, JobIdAndNotification, JobNotification, JobStoredData, NotificationData,
    NotificationId,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct PrimaryDb {
    pool: Pool<Sqlite>,
}

impl PrimaryDb {
    /// Opens primary DB "connection".
    pub async fn open<I: FnOnce() -> anyhow::Result<String>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(&initializer()?).await?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .with_context(|| "Failed to migrate database")?;

        Ok(PrimaryDb { pool })
    }

    /// Retrieves user from the `Users` table using user email.
    pub async fn get_user_by_email<T: AsRef<str>>(&self, email: T) -> anyhow::Result<Option<User>> {
        let email = email.as_ref();
        query_as!(
            RawUser,
            r#"
SELECT id, email, handle, credentials, created, roles, activated
FROM users
WHERE email = ?1
                "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .map(User::try_from)
        .transpose()
    }

    /// Retrieves user from the `Users` index using user handle.
    pub async fn get_user_by_handle<T: AsRef<str>>(
        &self,
        handle: T,
    ) -> anyhow::Result<Option<User>> {
        let handle = handle.as_ref();
        let mut raw_users = query_as!(
            RawUser,
            r#"
SELECT id, email, handle, credentials, created, roles, activated
FROM users
WHERE handle = ?1
             "#,
            handle
        )
        .fetch_all(&self.pool)
        .await?;

        if raw_users.is_empty() {
            return Ok(None);
        }

        if raw_users.len() > 1 {
            bail!(
                "Founds {} users for the same handle {}.",
                raw_users.len().to_string(),
                handle
            );
        }

        raw_users.pop().map(User::try_from).transpose()
    }

    /// Inserts user to the `Users` tables, fails if user already exists.
    pub async fn insert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let user_id: i64 = query_scalar!(
            r#"
INSERT INTO users (email, handle, credentials, created, roles, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.roles,
            raw_user.activated
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(UserId(user_id))
    }

    /// Inserts or updates user in the `Users` table.
    pub async fn upsert_user<U: AsRef<User>>(&self, user: U) -> anyhow::Result<UserId> {
        let raw_user = RawUserToUpsert::try_from(user.as_ref())?;

        let user_id: i64 = query_scalar!(r#"
INSERT INTO users (email, handle, credentials, created, roles, activated)
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
ON CONFLICT(email) DO UPDATE SET handle=excluded.handle, credentials=excluded.credentials, created=excluded.created, roles=excluded.roles, activated=excluded.activated
RETURNING id
        "#,
            raw_user.email,
            raw_user.handle,
            raw_user.credentials,
            raw_user.created,
            raw_user.roles,
            raw_user.activated
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(UserId(user_id))
    }

    /// Removes user with the specified email from the `Users` table.
    pub async fn remove_user_by_email<T: AsRef<str>>(
        &self,
        email: T,
    ) -> anyhow::Result<Option<User>> {
        let email = email.as_ref();
        query_as!(
            RawUser,
            r#"
DELETE FROM users
WHERE email = ?1
RETURNING id as "id!", email as "email!", handle as "handle!", credentials as "credentials!", created as "created!", roles, activated
            "#,
            email
        )
        .fetch_optional(&self.pool)
            .await?
            .map(User::try_from)
            .transpose()
    }

    /// Retrieves user data from the `UserData` table using user id and data key.
    pub async fn get_user_data<R: DeserializeOwned>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<Option<UserData<R>>> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        query_as!(
            RawUserData,
            r#"
SELECT value, timestamp
FROM user_data
WHERE user_id = ?1 AND namespace = ?2 AND key = ?3
                "#,
            user_id.0,
            namespace,
            key
        )
        .fetch_optional(&self.pool)
        .await?
        .map(UserData::try_from)
        .transpose()
    }

    /// Sets user data in the `UserData` table using user id and data key.
    pub async fn upsert_user_data<R: Serialize>(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
        user_data: UserData<R>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        let raw_user_data = RawUserData::try_from(&user_data)?;
        query!(
            r#"
INSERT INTO user_data (user_id, namespace, key, value, timestamp)
VALUES ( ?1, ?2, ?3, ?4, ?5 )
ON CONFLICT(user_id, namespace, key) DO UPDATE SET value=excluded.value, timestamp=excluded.timestamp
        "#,
            user_id.0,
            namespace,
            key,
            raw_user_data.value,
            raw_user_data.timestamp
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user data from the `UserData` table using user id and data key.
    pub async fn remove_user_data(
        &self,
        user_id: UserId,
        user_data_key: impl Into<UserDataKey<'_>>,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        query!(
            r#"
DELETE FROM user_data
WHERE user_id = ?1 AND namespace = ?2 AND key = ?3
            "#,
            user_id.0,
            namespace,
            key
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes user data from the `UserData` table with the specified data key if it's older than
    /// specified `since` timestamp.
    pub async fn cleanup_user_data(
        &self,
        user_data_key: impl Into<UserDataKey<'_>>,
        since: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let user_data_key = user_data_key.into();
        let namespace = user_data_key.namespace.as_ref();
        let key = user_data_key.key.unwrap_or_default();
        let since = since.unix_timestamp();
        query!(
            r#"
DELETE FROM user_data
WHERE namespace = ?1 AND key = ?2 AND timestamp <= ?3
            "#,
            namespace,
            key,
            since
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

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
WHERE email = ?1
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
        let raw_session_timestamp = session.timestamp.unix_timestamp();

        query!(
            r#"
INSERT INTO user_webauthn_sessions (email, session_value, timestamp)
VALUES (?1, ?2, ?3)
ON CONFLICT(email) DO UPDATE SET session_value=excluded.session_value, timestamp=excluded.timestamp
        "#,
            session.email,
            raw_session_value,
            raw_session_timestamp
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
WHERE email = ?1
            "#,
            email
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes WebAuthn sessions that are older than specified timestamp.
    pub async fn remove_user_webauthn_sessions(&self, since: OffsetDateTime) -> anyhow::Result<()> {
        let since = since.unix_timestamp();
        query!(
            r#"
DELETE FROM user_webauthn_sessions
WHERE timestamp <= ?1
            "#,
            since
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves all utils from the `Utils` table.
    pub async fn get_utils(&self) -> anyhow::Result<Vec<Util>> {
        let mut root_utils = query_as!(
            RawUtil,
            r#"
SELECT id, handle, name, keywords, parent_id
FROM utils
ORDER BY parent_id, id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Utilities are sorted by the parent_id meaning that all root utilities are returned first.
        let child_utils = if let Some(position) = root_utils
            .iter()
            .position(|raw_util| raw_util.parent_id.is_some())
        {
            root_utils.split_off(position)
        } else {
            return root_utils.into_iter().map(Util::try_from).collect();
        };

        let mut parent_children_map = HashMap::<_, Vec<_>>::new();
        for util in child_utils {
            if let Some(parent_id) = util.parent_id {
                parent_children_map.entry(parent_id).or_default().push(util);
            } else {
                bail!("Child utility does not have a parent id.");
            }
        }

        root_utils
            .into_iter()
            .map(|root_util| Self::build_util_tree(root_util, &mut parent_children_map))
            .collect()
    }

    /// Retrieves scheduler job from the `scheduler_jobs` table using Job ID.
    pub async fn get_scheduler_job(&self, id: Uuid) -> anyhow::Result<Option<JobStoredData>> {
        let id = id.hyphenated();
        query_as!(
            RawSchedulerJobStoredData,
            r#"
SELECT id as "id: uuid::fmt::Hyphenated", last_updated, next_tick, last_tick, job_type as "job_type!", count,
       ran, stopped, schedule, repeating, repeated_every, extra
FROM scheduler_jobs
WHERE id = ?1
                "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .map(JobStoredData::try_from)
        .transpose()
    }

    /// Upserts scheduler job to the `scheduler_jobs` table.
    pub async fn upsert_scheduler_job(&self, job: &JobStoredData) -> anyhow::Result<()> {
        let raw_job = RawSchedulerJobStoredData::try_from(job)?;

        query!(
            r#"
INSERT INTO scheduler_jobs (id, last_updated, next_tick, job_type, count, ran, stopped, schedule,
                            repeating, repeated_every, extra, last_tick)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
ON CONFLICT(id) DO UPDATE SET last_updated=excluded.last_updated, next_tick=excluded.next_tick,
                            job_type=excluded.job_type, count=excluded.count, ran=excluded.ran,
                            stopped=excluded.stopped, schedule=excluded.schedule,
                            repeating=excluded.repeating, repeated_every=excluded.repeated_every,
                            extra=excluded.extra, last_tick=excluded.last_tick
        "#,
            raw_job.id,
            raw_job.last_updated,
            raw_job.next_tick,
            raw_job.job_type,
            raw_job.count,
            raw_job.ran,
            raw_job.stopped,
            raw_job.schedule,
            raw_job.repeating,
            raw_job.repeated_every,
            raw_job.extra,
            raw_job.last_tick
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Removes scheduler job from the `scheduler_jobs` table using Job ID.
    pub async fn remove_scheduler_job(&self, id: Uuid) -> anyhow::Result<()> {
        let id = id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_jobs
WHERE id = ?1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves next scheduled jobs from `scheduler_jobs` table.
    pub async fn get_next_scheduler_jobs(&self) -> anyhow::Result<Vec<JobAndNextTick>> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let jobs = query_as!(
            RawSchedulerJobAndNextTick,
            r#"
SELECT id as "id: uuid::fmt::Hyphenated", job_type, next_tick, last_tick
FROM scheduler_jobs
WHERE next_tick > 0 AND next_tick < ?1
            "#,
            now
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs.into_iter().map(JobAndNextTick::from).collect())
    }

    /// Updates scheduler job ticks in the `scheduler_jobs` table.
    pub async fn set_scheduler_job_ticks(
        &self,
        id: Uuid,
        next_tick: Option<OffsetDateTime>,
        last_tick: Option<OffsetDateTime>,
    ) -> anyhow::Result<()> {
        let id = id.hyphenated();
        let next_tick = next_tick
            .map(|tick| tick.unix_timestamp())
            .unwrap_or_default();
        let last_tick = last_tick.map(|tick| tick.unix_timestamp());

        query!(
            r#"
UPDATE scheduler_jobs
SET next_tick=?2, last_tick=?3
WHERE id = ?1
        "#,
            id,
            next_tick,
            last_tick
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves time until the next scheduler job from the `scheduler_jobs` table.
    pub async fn get_scheduler_time_until_next_job(
        &self,
        since: OffsetDateTime,
    ) -> anyhow::Result<Option<Duration>> {
        let since = since.unix_timestamp();
        let next_tick = query!(
            r#"
SELECT next_tick
FROM scheduler_jobs
WHERE next_tick > 0 AND next_tick > ?
ORDER BY next_tick ASC
            "#,
            since
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(next_tick.and_then(|next_tick| {
            let next_tick = next_tick.next_tick? as u64;
            if next_tick > 0 {
                Some(Duration::from_secs(next_tick - since as u64))
            } else {
                None
            }
        }))
    }

    /// Retrieves scheduler notification from the `scheduler_notifications` table using Notification ID.
    pub async fn get_scheduler_notification(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<NotificationData>> {
        let id = id.hyphenated();
        let notification = query_as!(
            RawSchedulerNotification,
            r#"
SELECT job_id as "job_id: uuid::fmt::Hyphenated", extra
FROM scheduler_notifications
WHERE id = ?1
                "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        let notification = if let Some(notification) = notification {
            notification
        } else {
            return Ok(None);
        };

        let states = query!(
            r#"
SELECT state
FROM scheduler_notification_states
WHERE id = ?1
            "#,
            id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(NotificationData {
            job_id: Some(JobIdAndNotification {
                job_id: Some(notification.job_id.into_uuid().into()),
                notification_id: Some(id.into_uuid().into()),
            }),
            job_states: states
                .into_iter()
                .map(|record| record.state as i32)
                .collect(),
            extra: notification.extra.unwrap_or_default(),
        }))
    }

    /// Upserts scheduler notification to the `scheduler_notifications` table.
    pub async fn upsert_scheduler_notification(
        &self,
        notification: &NotificationData,
    ) -> anyhow::Result<()> {
        let (job_id, notification_id) = match notification.job_id_and_notification_id_from_data() {
            Some((job_id, notification_id)) => (job_id, notification_id),
            None => {
                bail!(
                    "Job ID and Notification ID are required for scheduler notification upsertion"
                );
            }
        };

        let notification_id = notification_id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notification_states
WHERE id = ?1
            "#,
            notification_id
        )
        .execute(&self.pool)
        .await?;

        let job_id = job_id.hyphenated();
        query!(
            r#"
INSERT INTO scheduler_notifications (id, job_id, extra)
VALUES (?1, ?2, ?3)
ON CONFLICT(id) DO UPDATE SET job_id=excluded.job_id, extra=excluded.extra
        "#,
            notification_id,
            job_id,
            notification.extra
        )
        .execute(&self.pool)
        .await?;

        if !notification.job_states.is_empty() {
            QueryBuilder::<Sqlite>::new("INSERT INTO scheduler_notification_states (id, state) ")
                .push_values(notification.job_states.iter(), |mut b, state| {
                    b.push_bind(notification_id).push_bind(*state as i64);
                })
                .build()
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification(&self, id: Uuid) -> anyhow::Result<()> {
        let id = id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notifications
WHERE id = ?1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieves notification ids from `scheduler_notifications` table.
    pub async fn get_scheduler_notification_ids_for_job_and_state(
        &self,
        job_id: JobId,
        state: JobNotification,
    ) -> anyhow::Result<Vec<NotificationId>> {
        let job_id = job_id.hyphenated();
        let state = state as i32;
        let notifications = query!(
            r#"
SELECT DISTINCT notifications.id as "id!: uuid::fmt::Hyphenated"
FROM scheduler_notifications as notifications
RIGHT JOIN scheduler_notification_states as states ON notifications.id = states.id
WHERE notifications.job_id = ?1 AND states.state = ?2
            "#,
            job_id,
            state
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(notifications
            .into_iter()
            .map(|notification| notification.id.into_uuid())
            .collect())
    }

    /// Retrieves notification ids from `scheduler_notifications` table.
    pub async fn get_scheduler_notification_ids_for_job(
        &self,
        job_id: JobId,
    ) -> anyhow::Result<Vec<NotificationId>> {
        let job_id = job_id.hyphenated();
        let notifications = query!(
            r#"
SELECT DISTINCT id as "id!: uuid::fmt::Hyphenated"
FROM scheduler_notifications
WHERE job_id = ?1
            "#,
            job_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(notifications
            .into_iter()
            .map(|notification| notification.id.into_uuid())
            .collect())
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification_for_state(
        &self,
        notification_id: Uuid,
        state: JobNotification,
    ) -> anyhow::Result<bool> {
        let notification_id = notification_id.hyphenated();
        let state = state as i32;
        let result = query!(
            r#"
DELETE FROM scheduler_notification_states
WHERE id = ?1 AND state = ?2
            "#,
            notification_id,
            state
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Removes scheduler notification from the `scheduler_notifications` table using notification ID.
    pub async fn remove_scheduler_notification_for_job(&self, job_id: Uuid) -> anyhow::Result<()> {
        let job_id = job_id.hyphenated();
        query!(
            r#"
DELETE FROM scheduler_notifications
WHERE job_id = ?1
            "#,
            job_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn build_util_tree(
        raw_util: RawUtil,
        parent_children_map: &mut HashMap<i64, Vec<RawUtil>>,
    ) -> anyhow::Result<Util> {
        let utils = if let Some(mut children) = parent_children_map.remove(&raw_util.id) {
            Some(
                children
                    .drain(..)
                    .map(|util| Self::build_util_tree(util, parent_children_map))
                    .collect::<anyhow::Result<_>>()?,
            )
        } else {
            None
        };

        Util::try_from(raw_util).map(|util| Util { utils, ..util })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::{StoredCredentials, WebAuthnSession, WebAuthnSessionValue},
        tests::{
            mock_db,
            webauthn::{SERIALIZED_AUTHENTICATION_STATE, SERIALIZED_REGISTRATION_STATE},
            MockUserBuilder,
        },
        users::{InternalUserDataNamespace, PublicUserDataNamespace, UserData, UserId},
    };
    use insta::assert_debug_snapshot;
    use std::{
        ops::{Add, Sub},
        time::Duration,
    };
    use time::OffsetDateTime;
    use tokio_cron_scheduler::{
        CronJob, JobIdAndNotification, JobNotification, JobStored, JobStoredData, JobType,
        NonCronJob, NotificationData,
    };
    use uuid::uuid;

    #[actix_rt::test]
    async fn can_add_and_retrieve_users() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("some-id").await?.is_none());

        let users = vec![
            MockUserBuilder::new(
                UserId::empty(),
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activated()
            .build(),
            MockUserBuilder::new(
                UserId::empty(),
                "prod@secutils.dev",
                "prod-handle",
                StoredCredentials {
                    password_hash: Some("hash_prod".to_string()),
                    ..Default::default()
                },
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .add_role("admin")
            .build(),
            MockUserBuilder::new(
                UserId::empty(),
                "user@secutils.dev",
                "handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .add_role("Power-User")
            .build(),
        ];
        for user in users {
            db.upsert_user(&user).await?;
        }

        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_by_email("prod@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    2,
                ),
                email: "prod@secutils.dev",
                handle: "prod-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash_prod",
                    ),
                    passkey: None,
                },
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_by_email("user@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    3,
                ),
                email: "user@secutils.dev",
                handle: "handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);
        assert!(db
            .get_user_by_email("unknown@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn ignores_email_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
            "DeV@secutils.dev",
            "DeV-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("Power-User")
        .set_activated()
        .build();
        let db = mock_db().await?;
        let id = db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?,  @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "DeV@secutils.dev",
                handle: "DeV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);
        assert_eq!(
            db.get_user_by_email("DEV@secutils.dev").await?.unwrap().id,
            id
        );
        assert_eq!(
            db.get_user_by_email("DeV@secutils.dev").await?.unwrap().id,
            id
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn ignores_handle_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            UserId::empty(),
            "DeV@secutils.dev",
            "DeV-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activated()
        .add_role("Power-User")
        .build();
        let db = mock_db().await?;
        let id = db.upsert_user(&user).await?;

        assert_debug_snapshot!(db.get_user_by_handle("dev-handle").await?,  @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "DeV@secutils.dev",
                handle: "DeV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);
        assert_eq!(db.get_user_by_handle("DEV-handle").await?.unwrap().id, id);
        assert_eq!(db.get_user_by_handle("DeV-handle").await?.unwrap().id, id);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_insert_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let user_id = db
            .insert_user(
                &MockUserBuilder::new(
                    UserId::empty(),
                    "dev@secutils.dev",
                    "dev-handle",
                    StoredCredentials {
                        password_hash: Some("hash".to_string()),
                        ..Default::default()
                    },
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                )
                .set_activated()
                .build(),
            )
            .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);

        let conflict_error = db
            .insert_user(
                &MockUserBuilder::new(
                    UserId(100),
                    "DEV@secutils.dev",
                    "DEV-handle",
                    StoredCredentials {
                        password_hash: Some("new-hash".to_string()),
                        ..Default::default()
                    },
                    // January 1, 2000 11:00:00
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                )
                .add_role("admin")
                .build(),
            )
            .await;
        assert_debug_snapshot!(conflict_error, @r###"
        Err(
            Database(
                SqliteError {
                    code: 2067,
                    message: "UNIQUE constraint failed: users.handle",
                },
            ),
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            user_id
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_user() -> anyhow::Result<()> {
        let db = mock_db().await?;

        db.upsert_user(
            &MockUserBuilder::new(
                UserId::empty(),
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activated()
            .build(),
        )
        .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "dev-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "hash",
                    ),
                    passkey: None,
                },
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                activated: true,
            },
        )
        "###);

        db.upsert_user(
            &MockUserBuilder::new(
                UserId(100),
                "DEV@secutils.dev",
                "DEV-handle",
                StoredCredentials {
                    password_hash: Some("new-hash".to_string()),
                    ..Default::default()
                },
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .add_role("admin")
            .build(),
        )
        .await?;
        assert_debug_snapshot!(db.get_user_by_email("dev@secutils.dev").await?, @r###"
        Some(
            User {
                id: UserId(
                    1,
                ),
                email: "dev@secutils.dev",
                handle: "DEV-handle",
                credentials: StoredCredentials {
                    password_hash: Some(
                        "new-hash",
                    ),
                    passkey: None,
                },
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                activated: false,
            },
        )
        "###);

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            db.get_user_by_email("DEV@secutils.dev").await?.unwrap().id
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_user() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());

        let user_dev = MockUserBuilder::new(
            UserId::empty(),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activated()
        .build();
        let user_prod = MockUserBuilder::new(
            UserId::empty(),
            "prod@secutils.dev",
            "prod-handle",
            StoredCredentials {
                password_hash: Some("hash_prod".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        let dev_user_id = db.upsert_user(&user_dev).await?;
        let prod_user_id = db.upsert_user(&user_prod).await?;

        assert_eq!(
            db.get_user_by_email("dev@secutils.dev").await?.unwrap().id,
            dev_user_id
        );
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            prod_user_id
        );

        assert_eq!(
            db.remove_user_by_email("dev@secutils.dev")
                .await?
                .unwrap()
                .id,
            dev_user_id
        );
        assert!(db.get_user_by_email("dev@secutils.dev").await?.is_none());
        assert!(db.remove_user_by_email("dev@secutils.dev").await?.is_none());
        assert_eq!(
            db.get_user_by_email("prod@secutils.dev").await?.unwrap().id,
            prod_user_id
        );

        assert_eq!(
            db.remove_user_by_email("prod@secutils.dev")
                .await?
                .unwrap()
                .id,
            prod_user_id
        );
        assert!(db.get_user_by_email("prod@secutils.dev").await?.is_none());
        assert!(db
            .remove_user_by_email("prod@secutils.dev")
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_manipulate_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;
        let user = MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .set_activated()
        .build();

        // No user and no data yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        db.upsert_user(&user).await?;

        // Nodata yet.
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        // Insert data.
        db.upsert_user_data(
            user.id,
            PublicUserDataNamespace::UserSettings,
            UserData::new("data", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                "data".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Update data.
        db.upsert_user_data(
            user.id,
            PublicUserDataNamespace::UserSettings,
            UserData::new("data-new", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            Some(UserData::new(
                "data-new".to_string(),
                OffsetDateTime::from_unix_timestamp(946720800)?
            ))
        );

        // Remove data.
        db.remove_user_data(user.id, PublicUserDataNamespace::UserSettings)
            .await?;
        assert_eq!(
            db.get_user_data::<String>(user.id, PublicUserDataNamespace::UserSettings)
                .await?,
            None
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_old_user_data() -> anyhow::Result<()> {
        let db = mock_db().await?;

        // Create test users
        let users = vec![
            MockUserBuilder::new(
                UserId(1),
                "dev@secutils.dev",
                "dev-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                OffsetDateTime::now_utc(),
            )
            .set_activated()
            .build(),
            MockUserBuilder::new(
                UserId(2),
                "prod@secutils.dev",
                "prod-handle",
                StoredCredentials {
                    password_hash: Some("hash".to_string()),
                    ..Default::default()
                },
                OffsetDateTime::now_utc(),
            )
            .set_activated()
            .build(),
        ];
        for user in users {
            db.upsert_user(&user).await?;
        }

        // Insert data for both users.
        db.upsert_user_data(
            UserId(1),
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2000 11:00:00
            UserData::new("data-1", OffsetDateTime::from_unix_timestamp(946720800)?),
        )
        .await?;
        db.upsert_user_data(
            UserId(2),
            InternalUserDataNamespace::AccountActivationToken,
            // January 1, 2010 11:00:00
            UserData::new("data-2", OffsetDateTime::from_unix_timestamp(1262340000)?),
        )
        .await?;

        // Check that data exists.
        assert_debug_snapshot!(db.get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
                value: "data-1",
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
                .await?, @r###"
        Some(
            UserData {
                value: "data-2",
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        // Run cleanup
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(946720800)?.sub(Duration::from_secs(60)),
        )
        .await?;

        // All data should still stay.
        assert!(db
            .get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(946720800)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(UserId(1), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_none());
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_some());

        // Run cleanup with another `since`.
        db.cleanup_user_data(
            InternalUserDataNamespace::AccountActivationToken,
            OffsetDateTime::from_unix_timestamp(1262340000)?.add(Duration::from_secs(60)),
        )
        .await?;
        assert!(db
            .get_user_data::<String>(UserId(2), InternalUserDataNamespace::AccountActivationToken)
            .await?
            .is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_webauthn_sessions() -> anyhow::Result<()> {
        let db = mock_db().await?;
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

    #[actix_rt::test]
    async fn ignores_email_case_for_webauthn_sessions() -> anyhow::Result<()> {
        let db = mock_db().await?;

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

    #[actix_rt::test]
    async fn can_update_webauthn_sessions() -> anyhow::Result<()> {
        let db = mock_db().await?;

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

    #[actix_rt::test]
    async fn can_remove_webauthn_session() -> anyhow::Result<()> {
        let db = mock_db().await?;
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

    #[actix_rt::test]
    async fn can_remove_old_webauthn_session() -> anyhow::Result<()> {
        let db = mock_db().await?;
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

    #[actix_rt::test]
    async fn can_add_and_retrieve_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946720800,
                ),
                last_tick: Some(
                    946720700,
                ),
                next_tick: 946720900,
                job_type: 0,
                count: 3,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("11e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946720800,
                ),
                last_tick: Some(
                    946720700,
                ),
                next_tick: 946720900,
                job_type: 0,
                count: 3,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        db.upsert_scheduler_job(&JobStoredData {
            id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
            last_updated: Some(946721800u64),
            last_tick: Some(946721700u64),
            next_tick: 946721900u64,
            count: 4,
            job_type: JobType::Cron as i32,
            extra: vec![1, 2, 3, 4, 5],
            ran: true,
            stopped: true,
            job: Some(JobStored::CronJob(CronJob {
                schedule: "0 0 0 1 1 * *".to_string(),
            })),
        })
        .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946721800,
                ),
                last_tick: Some(
                    946721700,
                ),
                next_tick: 946721900,
                job_type: 0,
                count: 4,
                extra: [
                    1,
                    2,
                    3,
                    4,
                    5,
                ],
                ran: true,
                stopped: true,
                job: Some(
                    CronJob(
                        CronJob {
                            schedule: "0 0 0 1 1 * *",
                        },
                    ),
                ),
            },
        )
        "###);

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @r###"
        Some(
            JobStoredData {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                last_updated: Some(
                    946820800,
                ),
                last_tick: Some(
                    946820700,
                ),
                next_tick: 946820900,
                job_type: 2,
                count: 0,
                extra: [
                    1,
                    2,
                    3,
                ],
                ran: true,
                stopped: false,
                job: Some(
                    NonCronJob(
                        NonCronJob {
                            repeating: false,
                            repeated_every: 0,
                        },
                    ),
                ),
            },
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 7486478208841368175,
                id2: 10540599508476092616,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 64546022934790767,
                id2: 10540599508476092616,
            },
        )
        "###);

        db.remove_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().id, @r###"
        Some(
            Uuid {
                id1: 7486478208841368175,
                id2: 10540599508476092616,
            },
        )
        "###);
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        db.remove_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert_debug_snapshot!(db.get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");
        assert_debug_snapshot!(db.get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8")).await?, @"None");

        db.remove_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_next_scheduler_jobs() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db.get_next_scheduler_jobs().await?.is_empty());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_debug_snapshot!(db.get_next_scheduler_jobs().await?, @r###"
        [
            JobAndNextTick {
                id: Some(
                    Uuid {
                        id1: 7486478208841368175,
                        id2: 10540599508476092616,
                    },
                ),
                job_type: 0,
                next_tick: 946720900,
                last_tick: Some(
                    946720700,
                ),
            },
            JobAndNextTick {
                id: Some(
                    Uuid {
                        id1: 64546022934790767,
                        id2: 10540599508476092616,
                    },
                ),
                job_type: 2,
                next_tick: 946820900,
                last_tick: Some(
                    946820700,
                ),
            },
        ]
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_job_ticks() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720900u64);
        assert_eq!(job.last_tick, Some(946720700u64));

        let job = db
            .get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946820900u64);
        assert_eq!(job.last_tick, Some(946820700u64));

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            None,
            Some(OffsetDateTime::from_unix_timestamp(946720704).unwrap()),
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 0);
        assert_eq!(job.last_tick, Some(946720704u64));

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946720903).unwrap()),
            None,
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720903);
        assert_eq!(job.last_tick, None);

        db.set_scheduler_job_ticks(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946720901).unwrap()),
            Some(OffsetDateTime::from_unix_timestamp(946720702).unwrap()),
        )
        .await?;

        db.set_scheduler_job_ticks(
            uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"),
            Some(OffsetDateTime::from_unix_timestamp(946820901).unwrap()),
            Some(OffsetDateTime::from_unix_timestamp(946820702).unwrap()),
        )
        .await?;

        let job = db
            .get_scheduler_job(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946720901u64);
        assert_eq!(job.last_tick, Some(946720702u64));

        let job = db
            .get_scheduler_job(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .unwrap();
        assert_eq!(job.next_tick, 946820901u64);
        assert_eq!(job.last_tick, Some(946820702u64));

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_scheduler_time_until_next_job() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_time_until_next_job(OffsetDateTime::now_utc())
            .await?
            .is_none());

        let jobs = vec![
            JobStoredData {
                id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946720800u64),
                last_tick: Some(946720700u64),
                next_tick: 946720900u64,
                count: 3,
                job_type: JobType::Cron as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::CronJob(CronJob {
                    schedule: "0 0 0 1 1 * *".to_string(),
                })),
            },
            JobStoredData {
                id: Some(uuid!("00e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                last_updated: Some(946820800u64),
                last_tick: Some(946820700u64),
                next_tick: 946820900u64,
                count: 0,
                job_type: JobType::OneShot as i32,
                extra: vec![1, 2, 3],
                ran: true,
                stopped: false,
                job: Some(JobStored::NonCronJob(NonCronJob {
                    repeating: false,
                    repeated_every: 0,
                })),
            },
        ];

        for job in jobs {
            db.upsert_scheduler_job(&job).await?;
        }

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946720800).unwrap()
            )
            .await?,
            Some(Duration::from_secs(100))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946730900).unwrap()
            )
            .await?,
            Some(Duration::from_secs(90000))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946820899).unwrap()
            )
            .await?,
            Some(Duration::from_secs(1))
        );

        assert_eq!(
            db.get_scheduler_time_until_next_job(
                OffsetDateTime::from_unix_timestamp(946820901).unwrap()
            )
            .await?,
            None
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn can_add_and_retrieve_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;
        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 568949181200286319,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7486478208841368175,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    1,
                    2,
                ],
                extra: [
                    1,
                    2,
                    3,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 154618015482200687,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7072147043123282543,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    0,
                    4,
                ],
                extra: [
                    4,
                    5,
                    6,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("11255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_update_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.upsert_scheduler_notification(&NotificationData {
            job_id: Some(JobIdAndNotification {
                job_id: Some(uuid!("17e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
            }),
            job_states: vec![JobNotification::Removed as i32],
            extra: vec![1, 2, 3, 4, 5],
        })
        .await?;

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 1721870685807133295,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7486478208841368175,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    4,
                ],
                extra: [
                    1,
                    2,
                    3,
                    4,
                    5,
                ],
            },
        )
        "###);

        assert_debug_snapshot!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        Some(
            NotificationData {
                job_id: Some(
                    JobIdAndNotification {
                        job_id: Some(
                            Uuid {
                                id1: 154618015482200687,
                                id2: 10540599508476092616,
                            },
                        ),
                        notification_id: Some(
                            Uuid {
                                id1: 7072147043123282543,
                                id2: 10540599508476092616,
                            },
                        ),
                    },
                ),
                job_states: [
                    0,
                    4,
                ],
                extra: [
                    4,
                    5,
                    6,
                ],
            },
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_scheduler_notifications() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_notification_ids_for_job_and_state() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Started)
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Scheduled)
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Stop)
            .await?, @r###"
        [
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Done)
            .await?, @"[]");

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Removed)
            .await?, @r###"
        [
            62255044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("03335044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Removed)
            .await?, @"[]");

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job_and_state(uuid!("00000044-10b1-426f-9247-bb680e5fe0c8"), JobNotification::Started)
            .await?, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_get_notification_ids_for_job() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        [
            67e55044-10b1-426f-9247-bb680e5fe0c8,
            77e55044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @r###"
        [
            62255044-10b1-426f-9247-bb680e5fe0c8,
        ]
        "###);

        assert_debug_snapshot!(db.get_scheduler_notification_ids_for_job(uuid!("00000044-10b1-426f-9247-bb680e5fe0c8"))
            .await?, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_notifications_for_state() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![JobNotification::Removed as i32],
                extra: vec![4, 5, 6],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Done,
        )
        .await?;
        db.remove_scheduler_notification_for_state(
            uuid!("00055044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Started,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            1,
            2,
        ]
        "###);
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Started,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            1,
        ]
        "###);
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Scheduled,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @r###"
        [
            4,
        ]
        "###);

        db.remove_scheduler_notification_for_state(
            uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"),
            JobNotification::Removed,
        )
        .await?;

        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");
        assert_debug_snapshot!(db.get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8")).await?.unwrap().job_states, @"[]");

        Ok(())
    }

    #[actix_rt::test]
    async fn can_remove_notifications_for_job() -> anyhow::Result<()> {
        let db = mock_db().await?;

        let notifications = vec![
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Scheduled as i32,
                ],
                extra: vec![1, 2, 3],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Stop as i32,
                    JobNotification::Removed as i32,
                ],
                extra: vec![4, 5, 6],
            },
            NotificationData {
                job_id: Some(JobIdAndNotification {
                    job_id: Some(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                    notification_id: Some(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8").into()),
                }),
                job_states: vec![
                    JobNotification::Started as i32,
                    JobNotification::Stop as i32,
                ],
                extra: vec![1, 2, 3],
            },
        ];

        for notification in notifications {
            db.upsert_scheduler_notification(&notification).await?;
        }

        db.remove_scheduler_notification_for_job(uuid!("67e00000-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());

        db.remove_scheduler_notification_for_job(uuid!("07e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_some());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        assert!(db
            .get_scheduler_notification(uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("62255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());
        assert!(db
            .get_scheduler_notification(uuid!("77e55044-10b1-426f-9247-bb680e5fe0c8"))
            .await?
            .is_none());

        db.remove_scheduler_notification_for_job(uuid!("02255044-10b1-426f-9247-bb680e5fe0c8"))
            .await?;

        Ok(())
    }
}
