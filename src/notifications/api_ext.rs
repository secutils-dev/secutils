use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport, EmailTransportError},
    notifications::{
        EmailNotificationAttachmentDisposition, EmailNotificationContent, Notification,
        NotificationContent, NotificationDestination, NotificationId,
    },
    users::{ResolvedRecipient, resolve_recipient_for_user_id, unsubscribe_url},
};
use anyhow::{Context, bail};
use futures::{StreamExt, pin_mut};
use lettre::{
    Message,
    message::{
        Attachment, MultiPart, SinglePart,
        header::{ContentType, HeaderName, HeaderValue},
    },
};
use std::cmp;
use time::OffsetDateTime;
use tracing::{error, info};

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
                    error!("Failed to send notification {}: {err:?}", *notification_id);
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
                // Resolve to a verified custom notification address when present, otherwise fall
                // back to the user's login email.
                let recipient = resolve_recipient_for_user_id(self.api, user_id).await?;

                // The unsubscribe URL is built once and threaded into both the rendered body
                // (visible footer) and the outgoing message (`List-Unsubscribe` /
                // `List-Unsubscribe-Post` headers). Tying both to the same source
                // (`recipient.unsubscribe_token`) guarantees the footer never appears without the
                // header (and vice versa) - Gmail's bulk-sender chip is unreliable for low-volume
                // senders, so the body link is the failsafe.
                let unsubscribe_url = recipient
                    .unsubscribe_token
                    .as_deref()
                    .map(|token| unsubscribe_url(self.api, token));
                self.send_email_notification(
                    recipient,
                    notification
                        .content
                        .into_email(self.api, unsubscribe_url.as_deref())
                        .await?,
                    notification.scheduled_at,
                )
                .await?;
            }
            NotificationDestination::Email(email_address) => {
                // Literal-address destinations (Kratos courier traffic, in-app verification
                // emails, etc.) bypass the resolver and never carry `List-Unsubscribe`
                // headers; they are transactional and exempt under RFC 8058.
                self.send_email_notification(
                    ResolvedRecipient {
                        address: email_address,
                        unsubscribe_token: None,
                    },
                    notification.content.into_email(self.api, None).await?,
                    notification.scheduled_at,
                )
                .await?;
            }
            NotificationDestination::ServerLog => {
                info!("Sending notification: {notification:?}");
            }
        }

        Ok(())
    }

    /// Send email notification using configured SMTP server.
    async fn send_email_notification(
        &self,
        recipient: ResolvedRecipient,
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

        let parsed_recipient = if let Some(catch_all_recipient) = catch_all_recipient {
            catch_all_recipient.parse().with_context(|| {
                format!("Cannot parse catch-all TO address: {catch_all_recipient}")
            })?
        } else {
            recipient
                .address
                .parse()
                .with_context(|| format!("Cannot parse TO address: {}", recipient.address))?
        };

        let mut message_builder = Message::builder()
            .from(smtp_config.username.parse()?)
            .reply_to(smtp_config.username.parse()?)
            .to(parsed_recipient)
            .subject(&email.subject)
            .date(timestamp.into());

        // RFC 8058 one-click unsubscribe headers, attached only when the recipient came from a
        // verified custom notification destination. Kratos courier mail (auth/recovery) and
        // the in-app verification flow itself never carry these headers; they are transactional
        // and explicitly exempt by the spec.
        if let Some(token) = &recipient.unsubscribe_token {
            let url = unsubscribe_url(self.api, token);
            let mailto = format!(
                "mailto:unsubscribe+{token}@{}",
                smtp_host(&smtp_config.username)
            );
            message_builder = message_builder
                .raw_header(HeaderValue::new(
                    HeaderName::new_from_ascii_str("List-Unsubscribe"),
                    format!("<{url}>, <{mailto}>"),
                ))
                .raw_header(HeaderValue::new(
                    HeaderName::new_from_ascii_str("List-Unsubscribe-Post"),
                    "List-Unsubscribe=One-Click".to_owned(),
                ));
        }

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

/// Extracts the host portion of an SMTP "user@host" address used for the one-click
/// `mailto:unsubscribe+token@host` fallback. Falls back to a placeholder when the configured
/// username is not in the standard "local@host" form (e.g. local relay setups), so we never
/// emit a malformed `mailto:` URI.
fn smtp_host(smtp_username: &str) -> &str {
    smtp_username
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or("localhost")
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
    use super::smtp_host;
    use crate::{
        config::{SmtpCatchAllConfig, SmtpConfig},
        notifications::{
            EmailNotificationAttachment, EmailNotificationContent, Notification,
            NotificationContent, NotificationContentTemplate, NotificationDestination,
        },
        tests::{mock_api, mock_api_with_config, mock_config, mock_user},
        users::{
            NotificationChannelKind,
            notification_destinations_tests::{PendingDestinationUpsert, verification_expiry},
        },
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

    #[test]
    fn smtp_host_extracts_host_from_username() {
        // Standard local@host form: pull off everything to the right of '@'.
        assert_eq!(smtp_host("user@example.com"), "example.com");
        assert_eq!(
            smtp_host("svc@host.subdomain.example.com"),
            "host.subdomain.example.com"
        );
        // The implementation uses `rsplit_once`, so multi-`@` strings keep the right-most host.
        assert_eq!(smtp_host("a@b@example.com"), "example.com");
        // Non-standard inputs (relay setups without a host part) fall back to a placeholder
        // to guarantee the emitted `mailto:unsubscribe+token@…` URI is at least syntactically
        // valid.
        assert_eq!(smtp_host("not-an-email"), "localhost");
        assert_eq!(smtp_host(""), "localhost");
    }

    /// Sets up a fully-verified custom notification email for `user_id` directly via the DB,
    /// bypassing the rate-limiter and verification flow that `NotificationDestinationsApi`
    /// would normally enforce. Returns the unsubscribe token persisted on the row so tests
    /// can assert `List-Unsubscribe` URLs verbatim.
    async fn setup_verified_email(
        db: &crate::database::Database,
        user_id: crate::users::UserId,
        address: &str,
        token: &str,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::from_unix_timestamp(1700000000)?;
        db.upsert_pending_notification_destination(PendingDestinationUpsert {
            user_id,
            kind: NotificationChannelKind::Email,
            address,
            verification_code_hash: "phc-test-hash",
            verification_expires_at: verification_expiry(now),
            verification_sent_at: now,
            unsubscribe_token: token,
            now,
        })
        .await?;
        db.mark_notification_destination_verified(user_id, NotificationChannelKind::Email, now)
            .await?;
        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_routes_to_verified_custom_email_with_unsubscribe_headers(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;
        setup_verified_email(&api.db, mock_user.id, "alerts@example.com", "tok-abc-123").await?;

        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 1);
        let (envelope, content) = &messages[0];

        // Routed to the verified custom address rather than the login email.
        assert_eq!(envelope.to().len(), 1);
        assert_eq!(envelope.to()[0].to_string(), "alerts@example.com");

        // Both `List-Unsubscribe` and `List-Unsubscribe-Post` headers are present, and the
        // URL form encodes the persisted unsubscribe token. Lettre may fold the header value
        // across CRLF + space so each URI is asserted independently rather than as one
        // contiguous string.
        assert!(
            content.contains("List-Unsubscribe:"),
            "missing List-Unsubscribe header in:\n{content}"
        );
        assert!(
            content
                .contains("<https://secutils.dev/api/notifications/unsubscribe?token=tok-abc-123>"),
            "missing HTTPS unsubscribe URL in List-Unsubscribe header:\n{content}"
        );
        assert!(
            content.contains("<mailto:unsubscribe+tok-abc-123@secutils.dev>"),
            "missing mailto unsubscribe URI in List-Unsubscribe header:\n{content}"
        );
        assert!(
            content.contains("List-Unsubscribe-Post: List-Unsubscribe=One-Click"),
            "missing List-Unsubscribe-Post header in:\n{content}"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_falls_back_to_login_when_no_custom_email_configured(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        assert_eq!(messages.len(), 1);
        let (envelope, content) = &messages[0];

        // Falls back to the user's login email — no custom destination configured.
        assert_eq!(
            envelope.to()[0].to_string(),
            "dev-00000000-0000-0000-0000-000000000001@secutils.dev"
        );
        // No unsubscribe headers when routing to the login email.
        assert!(
            !content.contains("List-Unsubscribe"),
            "login-email fallback must not carry List-Unsubscribe headers, got:\n{content}"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_falls_back_to_login_when_custom_email_unverified(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        // Pending (unverified) row — no `verified_at`.
        let now = OffsetDateTime::from_unix_timestamp(1700000000)?;
        api.db
            .upsert_pending_notification_destination(PendingDestinationUpsert {
                user_id: mock_user.id,
                kind: NotificationChannelKind::Email,
                address: "alerts@example.com",
                verification_code_hash: "phc-test-hash",
                verification_expires_at: verification_expiry(now),
                verification_sent_at: now,
                unsubscribe_token: "tok-pending",
                now,
            })
            .await?;

        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        let (envelope, content) = &messages[0];
        assert_eq!(
            envelope.to()[0].to_string(),
            "dev-00000000-0000-0000-0000-000000000001@secutils.dev",
            "unverified custom email must not be used as recipient"
        );
        assert!(
            !content.contains("List-Unsubscribe"),
            "unverified-fallback must not carry List-Unsubscribe headers"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_falls_back_to_login_when_custom_email_unsubscribed(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;
        setup_verified_email(&api.db, mock_user.id, "alerts@example.com", "tok-unsub").await?;

        // Unsubscribe via the same token; the row stays in the table but is filtered out
        // by `resolve_recipient_for_user_id`.
        let now = OffsetDateTime::from_unix_timestamp(1700001000)?;
        api.db
            .mark_notification_destination_unsubscribed("tok-unsub", now)
            .await?;

        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        let (envelope, content) = &messages[0];
        assert_eq!(
            envelope.to()[0].to_string(),
            "dev-00000000-0000-0000-0000-000000000001@secutils.dev",
            "unsubscribed custom email must not be used as recipient"
        );
        assert!(
            !content.contains("List-Unsubscribe"),
            "unsubscribed-fallback must not carry List-Unsubscribe headers"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_with_template_content_emits_body_footer_alongside_header(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;
        setup_verified_email(&api.db, mock_user.id, "alerts@example.com", "tok-paired").await?;

        // Use a real template - the visible-footer contract only kicks in for product-mail
        // templates, so a `Template`-backed notification is the right vehicle to assert the
        // wire output (the previous routing tests use literal `Email` content which has no
        // footer concept).
        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Template(NotificationContentTemplate::PageTrackerChanges {
                    tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_name: "tracker".to_string(),
                    content: Ok("content".to_string()),
                    diff: None,
                }),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        let (envelope, content) = &messages[0];
        assert_eq!(envelope.to()[0].to_string(), "alerts@example.com");

        // Body footer (HTML) is emitted alongside the RFC 8058 header. The HTML body is
        // base64-or-quoted-printable-encoded inside the wire content, so we can't just
        // grep raw markup; instead we check for stable substrings the encoder preserves
        // verbatim. The token is URL-encoded the same way `unsubscribe_url()` does.
        assert!(
            content.contains("List-Unsubscribe"),
            "header must accompany body footer"
        );
        assert!(
            content.contains("Unsubscribe"),
            "rendered HTML body must contain a visible Unsubscribe link"
        );
        assert!(
            content.contains("tok-paired"),
            "the verified destination's unsubscribe token must round-trip through both header and body URL"
        );
        // The plain-text alternative carries the matching footer - the `To unsubscribe,
        // visit:` copy is a stable, encoder-preserved marker because it has no special
        // characters that QP/base64 transforms into something else.
        assert!(
            content.contains("To unsubscribe, visit:"),
            "plain-text alternative must carry the matching footer line"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn user_destination_login_fallback_omits_body_footer(pool: PgPool) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;
        // No verified custom email - routes to login, no token, no footer, no header.

        api.notifications()
            .schedule_notification(
                NotificationDestination::User(mock_user.id),
                NotificationContent::Template(NotificationContentTemplate::PageTrackerChanges {
                    tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                    tracker_name: "tracker".to_string(),
                    content: Ok("content".to_string()),
                    diff: None,
                }),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        let (_, content) = &messages[0];

        assert!(
            !content.contains("List-Unsubscribe"),
            "no header on login-email fallback"
        );
        assert!(
            !content.contains("To unsubscribe, visit:"),
            "no plain-text footer on login-email fallback"
        );
        // The wire contains an "Unsubscribe" string only inside the HTML body footer block,
        // so its absence is a tighter check than scanning the whole message.
        assert!(
            !content.contains(r#"class=3D&quot;email-footer&quot;"#)
                && !content.contains(r#"class="email-footer""#)
                && !content.contains("class=3D\"email-footer\""),
            "no body footer wrapper on login-email fallback"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn email_destination_never_carries_unsubscribe_headers(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let api = mock_api(pool).await?;
        api.db.upsert_user(&mock_user).await?;

        // Even when the user *does* have a verified custom email configured, a literal
        // `Email` destination (Kratos courier mail, in-app verification) is transactional
        // and must bypass the resolver entirely — RFC 8058 explicitly exempts it.
        setup_verified_email(&api.db, mock_user.id, "alerts@example.com", "tok-mixed").await?;

        api.notifications()
            .schedule_notification(
                NotificationDestination::Email("ops@secutils.dev".to_string()),
                NotificationContent::Email(EmailNotificationContent::html(
                    "subject", "text", "html",
                )),
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .await?;
        assert_eq!(api.notifications().send_pending_notifications(1).await?, 1);

        let messages = api.network.email_transport.messages().await;
        let (envelope, content) = &messages[0];
        assert_eq!(envelope.to()[0].to_string(), "ops@secutils.dev");
        assert!(
            !content.contains("List-Unsubscribe"),
            "literal Email destinations are transactional — no List-Unsubscribe headers, got:\n{content}"
        );

        Ok(())
    }
}
