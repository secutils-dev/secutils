use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{
        EmailNotificationAttachmentDisposition, EmailNotificationContent, Notification,
        NotificationContent, NotificationDestination, NotificationId,
    },
};
use anyhow::{Context, anyhow, bail};
use futures::{StreamExt, pin_mut};
use lettre::{
    Message,
    message::{Attachment, MultiPart, SinglePart, header::ContentType},
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
                    .ok_or_else(|| anyhow!("User ({}) is not found.", *user_id))?;
                self.send_email_notification(
                    user.email,
                    notification.content.into_email(self.api).await?,
                    notification.scheduled_at,
                )
                .await?;
            }
            NotificationDestination::Email(email_address) => {
                self.send_email_notification(
                    email_address,
                    notification.content.into_email(self.api).await?,
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
        email: EmailNotificationContent,
        timestamp: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let smtp_config = if let Some(ref smtp_config) = self.api.config.as_ref().smtp {
            smtp_config
        } else {
            bail!("SMTP is not configured.");
        };

        let catch_all_recipient = smtp_config.catch_all.as_ref().and_then(|catch_all| {
            // Checks if the email text matches the regular expression specified in `text_matcher`.
            if catch_all.text_matcher.is_match(&email.text) {
                Some(catch_all.recipient.as_str())
            } else {
                None
            }
        });

        let recipient = if let Some(catch_all_recipient) = catch_all_recipient {
            catch_all_recipient.parse().with_context(|| {
                format!("Cannot parse catch-all TO address: {}", catch_all_recipient)
            })?
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
            Some(html) => {
                let alternative_builder = MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(email.text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    );
                message_builder.multipart(match email.attachments {
                    Some(attachments) if !attachments.is_empty() => {
                        let mut message_builder = MultiPart::mixed().multipart(alternative_builder);
                        for attachment in attachments {
                            let attachment_builder = match attachment.disposition {
                                EmailNotificationAttachmentDisposition::Inline(id) => {
                                    Attachment::new_inline(id)
                                }
                            };
                            message_builder = message_builder.singlepart(attachment_builder.body(
                                attachment.content,
                                ContentType::parse(&attachment.content_type)?,
                            ));
                        }
                        message_builder
                    }
                    _ => alternative_builder,
                })?
            }
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
        config::{SmtpCatchAllConfig, SmtpConfig},
        notifications::{
            EmailNotificationAttachment, EmailNotificationContent, Notification,
            NotificationContent, NotificationDestination,
        },
        tests::{mock_api, mock_api_with_config, mock_config, mock_user},
    };
    use insta::assert_debug_snapshot;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[sqlx::test]
    async fn properly_schedules_notification(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        assert!(api.db.get_notification(1.try_into()?).await?.is_none());

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::User(uuid!("00000000-0000-0000-0000-000000000001").into()),
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
                        00000000-0000-0000-0000-000000000001,
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
                        00000000-0000-0000-0000-000000000001,
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

    #[sqlx::test]
    async fn properly_sends_all_pending_notifications(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        let notifications = vec![
            Notification::new(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Text("abc".to_string()),
                OffsetDateTime::from_unix_timestamp(946720700)?,
            ),
            Notification::new(
                NotificationDestination::Email("some@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
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

        let boundary_regex = regex::Regex::new(r#"boundary="(.+)""#)?;
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
                            serialized: "dev-00000000-0000-0000-0000-000000000001@secutils.dev",
                            at_start: 40,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: dev-00000000-0000-0000-0000-000000000001@secutils.dev\r\nSubject: [NO SUBJECT]\r\nDate: Sat, 01 Jan 2000 09:58:20 +0000\r\nContent-Transfer-Encoding: 7bit\r\n\r\nabc",
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

    #[sqlx::test]
    async fn properly_sends_email_notifications_with_attachments(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        let notifications = vec![Notification::new(
            NotificationDestination::Email("some@secutils.dev".to_string()),
            NotificationContent::Email(EmailNotificationContent::html_with_attachments(
                "subject",
                "text",
                "<img src='cid:logo' />",
                vec![EmailNotificationAttachment::inline(
                    "logo",
                    "image/png",
                    vec![1, 2, 3],
                )],
            )),
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )];

        for notification in notifications.into_iter() {
            api.notifications()
                .schedule_notification(
                    notification.destination,
                    notification.content,
                    notification.scheduled_at,
                )
                .await?;
        }

        assert_eq!(api.notifications().send_pending_notifications(3).await?, 1);
        assert!(api.db.get_notification(1.try_into()?).await?.is_none());

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 1);

        let boundary_regex = regex::Regex::new(r#"boundary="(.+)""#)?;
        let messages = messages
            .into_iter()
            .map(|(envelope, content)| {
                let mut patched_content = content.clone();
                for (index, capture) in boundary_regex
                    .captures_iter(&content)
                    .flat_map(|captures| captures.iter().skip(1).collect::<Vec<_>>())
                    .filter_map(|capture| Some(capture?.as_str()))
                    .enumerate()
                {
                    patched_content =
                        patched_content.replace(capture, &format!("BOUNDARY_{index}"));
                }

                (envelope, patched_content)
            })
            .collect::<Vec<_>>();

        assert_debug_snapshot!(messages, @r###"
        [
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
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: some@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed;\r\n boundary=\"BOUNDARY_0\"\r\n\r\n--BOUNDARY_0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY_1\"\r\n\r\n--BOUNDARY_1\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\ntext\r\n--BOUNDARY_1\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\n<img src='cid:logo' />\r\n--BOUNDARY_1--\r\n--BOUNDARY_0\r\nContent-ID: <logo>\r\nContent-Disposition: inline\r\nContent-Type: image/png\r\nContent-Transfer-Encoding: 7bit\r\n\r\n\u{1}\u{2}\u{3}\r\n--BOUNDARY_0--\r\n",
            ),
        ]
        "###);

        Ok(())
    }

    #[sqlx::test]
    async fn properly_sends_pending_notifications_in_batches(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
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
            assert!(
                api.db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_some()
            );
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
            assert!(
                api.db
                    .get_notification((n + 1).try_into()?)
                    .await?
                    .is_none()
            );
        }

        Ok(())
    }

    #[sqlx::test]
    async fn sends_email_notifications_respecting_catch_all_filter(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mut config = mock_config()?;
        let text_matcher = regex::Regex::new("(one text)|(two text)")?;
        config.smtp = config.smtp.map(|smtp| SmtpConfig {
            catch_all: Some(SmtpCatchAllConfig {
                recipient: "catch-all@secutils.dev".to_string(),
                text_matcher,
            }),
            ..smtp
        });
        let api = mock_api_with_config(pool, config).await?;
        api.db.upsert_user(&mock_user).await?;

        let notifications = vec![
            Notification::new(
                NotificationDestination::Email("one@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some one text message",
                    "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::Email("two@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some two text message",
                    "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::Email("three@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some three text message",
                    "html",
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

        assert_eq!(api.notifications().send_pending_notifications(4).await?, 3);

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 3);

        let boundary_regex = regex::Regex::new(r#"boundary="(.+)""#)?;
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
                            serialized: "catch-all@secutils.dev",
                            at_start: 9,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: catch-all@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome one text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "catch-all@secutils.dev",
                            at_start: 9,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: catch-all@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome two text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "three@secutils.dev",
                            at_start: 5,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: three@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome three text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
        ]
        "###);

        Ok(())
    }

    #[sqlx::test]
    async fn sends_email_notifications_respecting_wide_open_catch_all_filter(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let mut config = mock_config()?;
        let text_matcher = regex::Regex::new(".*")?;
        config.smtp = config.smtp.map(|smtp| SmtpConfig {
            catch_all: Some(SmtpCatchAllConfig {
                recipient: "catch-all@secutils.dev".to_string(),
                text_matcher,
            }),
            ..smtp
        });
        let api = mock_api_with_config(pool, config).await?;
        api.db.upsert_user(&mock_user).await?;

        let notifications = vec![
            Notification::new(
                NotificationDestination::Email("one@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some one text message",
                    "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::Email("two@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some two text message",
                    "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            ),
            Notification::new(
                NotificationDestination::Email("three@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject",
                    "some three text message",
                    "html",
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

        assert_eq!(api.notifications().send_pending_notifications(4).await?, 3);

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 3);

        let boundary_regex = regex::Regex::new(r#"boundary="(.+)""#)?;
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
                            serialized: "catch-all@secutils.dev",
                            at_start: 9,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: catch-all@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome one text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "catch-all@secutils.dev",
                            at_start: 9,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: catch-all@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome two text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
            (
                Envelope {
                    forward_path: [
                        Address {
                            serialized: "catch-all@secutils.dev",
                            at_start: 9,
                        },
                    ],
                    reverse_path: Some(
                        Address {
                            serialized: "dev@secutils.dev",
                            at_start: 3,
                        },
                    ),
                },
                "From: dev@secutils.dev\r\nReply-To: dev@secutils.dev\r\nTo: catch-all@secutils.dev\r\nSubject: subject\r\nDate: Sat, 01 Jan 2000 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative;\r\n boundary=\"BOUNDARY\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nsome three text message\r\n--BOUNDARY\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nhtml\r\n--BOUNDARY--\r\n",
            ),
        ]
        "###);

        Ok(())
    }
}
