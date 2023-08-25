use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{
        notification_content::NotificationEmailContent, Notification, NotificationContent,
        NotificationDestination, NotificationId,
    },
};
use anyhow::{anyhow, bail, Context};
use futures::{pin_mut, StreamExt};
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    Message,
};
use std::cmp;
use time::OffsetDateTime;

/// Defines a maximum number of notifications that can be retrieved from the database at once.
const MAX_NOTIFICATIONS_PAGE_SIZE: usize = 100;

/// Describes the API to work with notifications.
pub struct NotificationsApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> NotificationsApi<'a, DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Creates Notifications API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Schedules a new notification.
    pub async fn schedule_notification(
        &self,
        destination: NotificationDestination,
        content: NotificationContent,
        scheduled_at: OffsetDateTime,
    ) -> anyhow::Result<NotificationId> {
        self.api
            .db
            .insert_notification(&Notification::new(destination, content, scheduled_at))
            .await
    }

    /// Sends pending notifications. The max number to send is limited by `limit`.
    pub async fn send_pending_notifications(&self, limit: usize) -> anyhow::Result<usize> {
        let pending_notification_ids = self.api.db.get_notification_ids(
            OffsetDateTime::now_utc(),
            cmp::min(MAX_NOTIFICATIONS_PAGE_SIZE, limit),
        );
        pin_mut!(pending_notification_ids);

        let mut sent_notifications = 0;
        while let Some(notification_id) = pending_notification_ids.next().await {
            if let Some(notification) = self.api.db.get_notification(notification_id?).await? {
                let notification_id = notification.id;
                if let Err(err) = self.send_notification(notification).await {
                    log::error!(
                        "Failed to send notification {}: {:?}",
                        *notification_id,
                        err
                    );
                } else {
                    sent_notifications += 1;
                    self.api.db.remove_notification(notification_id).await?;
                }
            }

            if sent_notifications >= limit {
                break;
            }
        }

        Ok(sent_notifications)
    }

    /// Sends notification and removes it from the database, if it was sent successfully.
    async fn send_notification(&self, notification: Notification) -> anyhow::Result<()> {
        match notification.destination {
            NotificationDestination::User(user_id) => {
                let user = self
                    .api
                    .users()
                    .get(user_id)
                    .await?
                    .ok_or_else(|| anyhow!("User with ID `{}` is not found.", *user_id))?;
                self.send_email_notification(
                    user.email,
                    notification.content.into(),
                    notification.scheduled_at,
                )
                .await?;
            }
            NotificationDestination::Email(email_address) => {
                self.send_email_notification(
                    email_address,
                    notification.content.into(),
                    notification.scheduled_at,
                )
                .await?;
            }
            NotificationDestination::ServerLog => {
                log::info!("Sending notification: {:?}", notification);
            }
        }

        Ok(())
    }

    /// Send email notification using configured SMTP server.
    async fn send_email_notification(
        &self,
        recipient: String,
        email: NotificationEmailContent,
        timestamp: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let smtp_config = if let Some(ref smtp_config) = self.api.config.as_ref().smtp {
            smtp_config
        } else {
            bail!("SMTP is not configured.");
        };

        let recipient = if let Some(ref catch_all) = smtp_config.catch_all_recipient {
            catch_all.parse()?
        } else {
            recipient
                .parse()
                .with_context(|| format!("Cannot parse TO address: {}", recipient))?
        };

        let message_builder = Message::builder()
            .from(smtp_config.username.parse()?)
            .reply_to(smtp_config.username.parse()?)
            .to(recipient)
            .subject(&email.subject)
            .date(timestamp.into());

        let message = match email.html {
            Some(html) => message_builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(email.text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            )?,
            None => message_builder.body(email.text)?,
        };

        self.api.network.email_transport.send(message).await?;

        Ok(())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET>
where
    ET::Error: EmailTransportError,
{
    /// Returns an API to work with notifications.
    pub fn notifications(&self) -> NotificationsApi<'_, DR, ET> {
        NotificationsApi::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        notifications::{
            Notification, NotificationContent, NotificationDestination, NotificationEmailContent,
        },
        tests::{mock_api, mock_user},
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn properly_schedules_notification() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api().await?;
        api.db.upsert_user(&mock_user).await?;

        assert!(api.db.get_notification(1.try_into()?).await?.is_none());

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::User(123.try_into()?),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications.into_iter() {
            api.notifications()
                .schedule_notification(
                    notification.destination,
                    notification.content,
                    notification.scheduled_at,
                )
                .await?;
        }

        assert_debug_snapshot!(api.db.get_notification(1.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    1,
                ),
                destination: User(
                    UserId(
                        123,
                    ),
                ),
                content: Text(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(api.db.get_notification(2.try_into()?).await?, @r###"
        Some(
            Notification {
                id: NotificationId(
                    2,
                ),
                destination: User(
                    UserId(
                        123,
                    ),
                ),
                content: Text(
                    "abc",
                ),
                scheduled_at: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(api.db.get_notification(3.try_into()?).await?, @"None");

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_sends_all_pending_notifications() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api().await?;
        api.db.upsert_user(&mock_user).await?;

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720700)?,
            ),
            Notification::new(
                NotificationDestination::Email("some@secutils.dev".to_string()),
                NotificationContent::Email(NotificationEmailContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
        ];

        for notification in notifications.into_iter() {
            api.notifications()
                .schedule_notification(
                    notification.destination,
                    notification.content,
                    notification.scheduled_at,
                )
                .await?;
        }

        assert!(api.db.get_notification(1.try_into()?).await?.is_some());
        assert!(api.db.get_notification(2.try_into()?).await?.is_some());

        assert_eq!(api.notifications().send_pending_notifications(3).await?, 2);

        assert!(api.db.get_notification(1.try_into()?).await?.is_none());
        assert!(api.db.get_notification(2.try_into()?).await?.is_none());

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 2);

        let boundary_regex = regex::Regex::new(r#"boundary=\"(.+)\""#)?;
        let messages = messages
            .into_iter()
            .map(|(envelope, content)| {
                let boundary = boundary_regex
                    .captures(&content)
                    .and_then(|captures| captures.get(1))
                    .map(|capture| capture.as_str());

                (
                    envelope,
                    if let Some(boundary) = boundary {
                        content.replace(boundary, "BOUNDARY")
                    } else {
                        content
                    },
                )
            })
            .collect::<Vec<_>>();

        assert_debug_snapshot!(messages, @r###"
        [
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: dev@secutils.dev\r\nSubject: [NO SUBJECT]\r\nDate: Sat, 01 Jan 2000 09:58:20 +0000\r\nContent-Transfer-Encoding: 7bit\r\n\r\nabc",
            ),
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "some@secutils.dev",
                            at_start: 4,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: some@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\ntext\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
        ]
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_sends_pending_notifications_in_batches() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api().await?;
        api.db.upsert_user(&mock_user).await?;

        for n in 0..=9 {
            api.notifications()
                .schedule_notification(
                    NotificationDestination::User(mock_user.id),
                    NotificationContent::Text(format!("{}", n)),
                    OffsetDateTime::from_unix_timestamp(946720800 + n)?,
                )
                .await?;
        }

        for n in 0..=9 {
            assert!(api
                .db
                .get_notification((n + 1).try_into()?)
                .await?
                .is_some());
        }

        assert_eq!(api.notifications().send_pending_notifications(3).await?, 3);

        for n in 0..=9 {
            assert_eq!(
                api.db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_some(),
                n >= 3
            );
        }

        assert_eq!(api.notifications().send_pending_notifications(3).await?, 3);

        for n in 0..=9 {
            assert_eq!(
                api.db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_some(),
                n >= 6
            );
        }

        assert_eq!(api.notifications().send_pending_notifications(10).await?, 4);

        for n in 0..=9 {
            assert!(api
                .db
                .get_notification((n + 1).try_into()?)
                .await?
                .is_none());
        }

        Ok(())
    }
}
