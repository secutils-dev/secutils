use crate::{
    database::Database,
    users::{
        UserId,
        notification_destinations::{NotificationChannelKind, UserNotificationDestination},
    },
};
use sqlx::{query, query_as};
use time::OffsetDateTime;
use uuid::Uuid;

/// `query_as!` target that mirrors the row shape of `user_notification_destinations`. Kept
/// private to this module: callers always receive [`UserNotificationDestination`] (the domain
/// type with a parsed [`NotificationChannelKind`]).
#[derive(Debug, Clone)]
struct RawNotificationDestination {
    id: Uuid,
    user_id: Uuid,
    kind: String,
    address: String,
    config: serde_json::Value,
    verified_at: Option<OffsetDateTime>,
    verification_code_hash: Option<String>,
    verification_expires_at: Option<OffsetDateTime>,
    verification_sent_at: Option<OffsetDateTime>,
    verification_attempts: i32,
    unsubscribe_token: String,
    unsubscribed_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<RawNotificationDestination> for UserNotificationDestination {
    type Error = anyhow::Error;

    fn try_from(raw: RawNotificationDestination) -> anyhow::Result<Self> {
        Ok(Self {
            id: raw.id,
            user_id: raw.user_id.into(),
            kind: NotificationChannelKind::from_db_str(&raw.kind)?,
            address: raw.address,
            config: raw.config,
            verification_code_hash: raw.verification_code_hash,
            verification_attempts: raw.verification_attempts,
            unsubscribe_token: raw.unsubscribe_token,
            verified_at: raw.verified_at,
            verification_expires_at: raw.verification_expires_at,
            verification_sent_at: raw.verification_sent_at,
            unsubscribed_at: raw.unsubscribed_at,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

/// Window of time a verification code is accepted.
const VERIFICATION_TTL_SECONDS: i64 = 15 * 60;

/// A trimmed update payload used by [`Database::upsert_pending_notification_destination`]; keeps
/// the call site readable instead of passing eight positional arguments.
pub(crate) struct PendingDestinationUpsert<'a> {
    pub user_id: UserId,
    pub kind: NotificationChannelKind,
    pub address: &'a str,
    pub verification_code_hash: &'a str,
    pub verification_expires_at: OffsetDateTime,
    pub verification_sent_at: OffsetDateTime,
    pub unsubscribe_token: &'a str,
    pub now: OffsetDateTime,
}

/// Notification-destination CRUD on top of the primary database.
impl Database {
    /// Returns every notification destination configured for `user_id`. v1 expects 0 or 1 row
    /// per user (the schema enforces uniqueness on `(user_id, kind)` and only the email kind
    /// ships); the plural shape exists so future channels (Slack, PagerDuty) drop in cleanly.
    pub async fn get_user_notification_destinations(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<UserNotificationDestination>> {
        let rows = query_as!(
            RawNotificationDestination,
            r#"
SELECT id,
       user_id,
       kind,
       address,
       config,
       verified_at,
       verification_code_hash,
       verification_expires_at,
       verification_sent_at,
       verification_attempts,
       unsubscribe_token,
       unsubscribed_at,
       created_at,
       updated_at
FROM user_notification_destinations
WHERE user_id = $1
ORDER BY kind ASC
            "#,
            *user_id
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(UserNotificationDestination::try_from)
            .collect()
    }

    /// Inserts a fresh row, or replaces the existing (user, kind) row, with a pending
    /// verification code. Always sets `verified_at = NULL`, `verification_attempts = 0`,
    /// `unsubscribed_at = NULL` so a re-issued code is a clean-slate state.
    pub async fn upsert_pending_notification_destination(
        &self,
        params: PendingDestinationUpsert<'_>,
    ) -> anyhow::Result<()> {
        query!(
            r#"
INSERT INTO user_notification_destinations (
    user_id, kind, address, verified_at, verification_code_hash, verification_expires_at,
    verification_sent_at, verification_attempts, unsubscribe_token, unsubscribed_at,
    created_at, updated_at
)
VALUES ($1, $2, $3, NULL, $4, $5, $6, 0, $7, NULL, $8, $8)
ON CONFLICT (user_id, kind) DO UPDATE SET
    address = EXCLUDED.address,
    verified_at = NULL,
    verification_code_hash = EXCLUDED.verification_code_hash,
    verification_expires_at = EXCLUDED.verification_expires_at,
    verification_sent_at = EXCLUDED.verification_sent_at,
    verification_attempts = 0,
    unsubscribed_at = NULL,
    -- The unsubscribe token is *not* rotated on every re-issue; it stays valid across the
    -- verify/unsubscribe lifecycle of the same row. We only generate a fresh one when the row
    -- is created, or when the address changes. Rotating it on every code resend would
    -- invalidate any in-flight `List-Unsubscribe` headers users already received.
    unsubscribe_token = CASE
        WHEN user_notification_destinations.address = EXCLUDED.address
        THEN user_notification_destinations.unsubscribe_token
        ELSE EXCLUDED.unsubscribe_token
    END,
    updated_at = EXCLUDED.updated_at
            "#,
            *params.user_id,
            params.kind.as_db_str(),
            params.address,
            params.verification_code_hash,
            params.verification_expires_at,
            params.verification_sent_at,
            params.unsubscribe_token,
            params.now,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Marks the destination as verified, clearing the verification fields.
    pub async fn mark_notification_destination_verified(
        &self,
        user_id: UserId,
        kind: NotificationChannelKind,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        query!(
            r#"
UPDATE user_notification_destinations
SET verified_at = $3,
    verification_code_hash = NULL,
    verification_expires_at = NULL,
    verification_attempts = 0,
    updated_at = $3
WHERE user_id = $1 AND kind = $2
            "#,
            *user_id,
            kind.as_db_str(),
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Increments the attempt counter on a failed code entry.
    pub async fn increment_notification_destination_attempts(
        &self,
        user_id: UserId,
        kind: NotificationChannelKind,
        now: OffsetDateTime,
    ) -> anyhow::Result<i32> {
        let row = query!(
            r#"
UPDATE user_notification_destinations
SET verification_attempts = verification_attempts + 1,
    updated_at = $3
WHERE user_id = $1 AND kind = $2
RETURNING verification_attempts
            "#,
            *user_id,
            kind.as_db_str(),
            now,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.verification_attempts)
    }

    /// Clears the active verification code (lockout case). Address remains so the user can
    /// retry with a fresh code via the resend flow.
    pub async fn clear_notification_destination_code(
        &self,
        user_id: UserId,
        kind: NotificationChannelKind,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        query!(
            r#"
UPDATE user_notification_destinations
SET verification_code_hash = NULL,
    verification_expires_at = NULL,
    updated_at = $3
WHERE user_id = $1 AND kind = $2
            "#,
            *user_id,
            kind.as_db_str(),
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Removes the destination row entirely.
    pub async fn delete_notification_destination(
        &self,
        user_id: UserId,
        kind: NotificationChannelKind,
    ) -> anyhow::Result<()> {
        query!(
            r#"
DELETE FROM user_notification_destinations
WHERE user_id = $1 AND kind = $2
            "#,
            *user_id,
            kind.as_db_str(),
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Looks up a destination by its public unsubscribe token. Used by the public RFC 8058
    /// endpoint, which has no authentication.
    pub async fn get_notification_destination_by_unsubscribe_token(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<UserNotificationDestination>> {
        let row = query_as!(
            RawNotificationDestination,
            r#"
SELECT id,
       user_id,
       kind,
       address,
       config,
       verified_at,
       verification_code_hash,
       verification_expires_at,
       verification_sent_at,
       verification_attempts,
       unsubscribe_token,
       unsubscribed_at,
       created_at,
       updated_at
FROM user_notification_destinations
WHERE unsubscribe_token = $1
            "#,
            token,
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(UserNotificationDestination::try_from).transpose()
    }

    /// Marks a destination as unsubscribed. Idempotent: setting `unsubscribed_at` on a row that
    /// is already unsubscribed is a no-op.
    pub async fn mark_notification_destination_unsubscribed(
        &self,
        token: &str,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        query!(
            r#"
UPDATE user_notification_destinations
SET unsubscribed_at = COALESCE(unsubscribed_at, $2),
    updated_at = $2
WHERE unsubscribe_token = $1
            "#,
            token,
            now,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Returns the standard verification expiry timestamp for `now`.
pub(crate) fn verification_expiry(now: OffsetDateTime) -> OffsetDateTime {
    now + time::Duration::seconds(VERIFICATION_TTL_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::{PendingDestinationUpsert, verification_expiry};
    use crate::{
        database::Database, tests::mock_user,
        users::notification_destinations::NotificationChannelKind,
    };
    use sqlx::PgPool;
    use time::OffsetDateTime;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1700000000).unwrap()
    }

    async fn email_destination(
        db: &Database,
        user_id: crate::users::UserId,
    ) -> anyhow::Result<Option<crate::users::UserNotificationDestination>> {
        Ok(db
            .get_user_notification_destinations(user_id)
            .await?
            .into_iter()
            .find(|d| d.kind == NotificationChannelKind::Email))
    }

    #[sqlx::test]
    async fn empty_for_new_user(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        let destinations = db.get_user_notification_destinations(user.id).await?;
        assert!(destinations.is_empty());
        Ok(())
    }

    #[sqlx::test]
    async fn upsert_then_verify(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc-hash-1",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "tok-1",
            now: now(),
        })
        .await?;

        let dest = email_destination(&db, user.id).await?.unwrap();
        assert_eq!(dest.address, "alerts@example.com");
        assert!(dest.verified_at.is_none());
        assert_eq!(dest.verification_attempts, 0);
        assert_eq!(dest.unsubscribe_token, "tok-1");

        // Verifying clears the code.
        db.mark_notification_destination_verified(user.id, NotificationChannelKind::Email, now())
            .await?;
        let dest = email_destination(&db, user.id).await?.unwrap();
        assert!(dest.is_verified());
        assert!(dest.verification_code_hash.is_none());

        let records = db.get_user_notification_destinations(user.id).await?;
        assert_eq!(records.len(), 1);
        assert!(records[0].is_verified());
        assert!(!records[0].has_pending_verification(now()));
        Ok(())
    }

    #[sqlx::test]
    async fn upsert_resend_keeps_unsubscribe_token(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc-hash-1",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "tok-1",
            now: now(),
        })
        .await?;

        // Same address, new code: token stays the same.
        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc-hash-2",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "tok-2-ignored",
            now: now(),
        })
        .await?;
        let dest = email_destination(&db, user.id).await?.unwrap();
        assert_eq!(dest.unsubscribe_token, "tok-1");
        assert_eq!(dest.verification_code_hash.unwrap(), "phc-hash-2");

        // Different address: token rotates.
        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "ops@example.com",
            verification_code_hash: "phc-hash-3",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "tok-3",
            now: now(),
        })
        .await?;
        let dest = email_destination(&db, user.id).await?.unwrap();
        assert_eq!(dest.address, "ops@example.com");
        assert_eq!(dest.unsubscribe_token, "tok-3");
        Ok(())
    }

    #[sqlx::test]
    async fn unsubscribe_lookup_and_idempotent_mark(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "unsub-token-xyz",
            now: now(),
        })
        .await?;
        db.mark_notification_destination_verified(user.id, NotificationChannelKind::Email, now())
            .await?;

        let by_token = db
            .get_notification_destination_by_unsubscribe_token("unsub-token-xyz")
            .await?
            .unwrap();
        assert_eq!(by_token.address, "alerts@example.com");
        assert!(by_token.unsubscribed_at.is_none());

        db.mark_notification_destination_unsubscribed("unsub-token-xyz", now())
            .await?;
        let after = db
            .get_notification_destination_by_unsubscribe_token("unsub-token-xyz")
            .await?
            .unwrap();
        assert_eq!(after.unsubscribed_at, Some(now()));

        // Bogus token returns None.
        assert!(
            db.get_notification_destination_by_unsubscribe_token("bogus")
                .await?
                .is_none()
        );

        // Idempotent: second mark does not move the timestamp.
        let later = OffsetDateTime::from_unix_timestamp(1700001000).unwrap();
        db.mark_notification_destination_unsubscribed("unsub-token-xyz", later)
            .await?;
        let final_state = db
            .get_notification_destination_by_unsubscribe_token("unsub-token-xyz")
            .await?
            .unwrap();
        assert_eq!(final_state.unsubscribed_at, Some(now()));
        Ok(())
    }

    #[sqlx::test]
    async fn cascade_on_user_delete(pool: PgPool) -> anyhow::Result<()> {
        let db = Database::create(pool).await?;
        let user = mock_user()?;
        db.upsert_user(&user).await?;

        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id: user.id,
            kind: NotificationChannelKind::Email,
            address: "alerts@example.com",
            verification_code_hash: "phc",
            verification_expires_at: verification_expiry(now()),
            verification_sent_at: now(),
            unsubscribe_token: "tok",
            now: now(),
        })
        .await?;
        assert_eq!(
            db.get_user_notification_destinations(user.id).await?.len(),
            1
        );

        db.remove_user_by_email(&user.email).await?;
        assert!(
            db.get_user_notification_destinations(user.id)
                .await?
                .is_empty()
        );
        Ok(())
    }
}
