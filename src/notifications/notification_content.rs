use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{EmailNotificationContent, NotificationContentTemplate},
};
use serde::{Deserialize, Serialize};

/// Describes the content of a notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContent {
    /// Notification content is represented as a custom string.
    Text(String),
    /// Notification content is represented as a custom email.
    Email(EmailNotificationContent),
    /// Notification content is represented as a template.
    Template(NotificationContentTemplate),
}

impl NotificationContent {
    /// Consumes notification content and return its email representation if supported.
    pub async fn into_email<DR: DnsResolver, ET: EmailTransport>(
        self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<EmailNotificationContent> {
        Ok(match self {
            NotificationContent::Text(text) => EmailNotificationContent::text("[NO SUBJECT]", text),
            NotificationContent::Email(email) => email,
            NotificationContent::Template(template) => template.compile_to_email(api).await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EmailNotificationContent, NotificationContent};
    use crate::{
        notifications::{
            AccountActivationTemplate, EmailNotificationAttachment, NotificationContentTemplate,
        },
        tests::{mock_api, mock_user},
        users::{InternalUserDataNamespace, UserData},
    };
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Text("abc".to_string()))?,
            vec![0, 3, 97, 98, 99]
        );

        assert_eq!(
            postcard::to_stdvec(&NotificationContent::Email(EmailNotificationContent::text(
                "abc", "def"
            )))?,
            vec![1, 3, 97, 98, 99, 3, 100, 101, 102, 0, 0]
        );
        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationContent>(&[0, 3, 97, 98, 99])?,
            NotificationContent::Text("abc".to_string())
        );

        assert_eq!(
            postcard::from_bytes::<NotificationContent>(&[
                1, 3, 97, 98, 99, 3, 100, 101, 102, 0, 0
            ])?,
            NotificationContent::Email(EmailNotificationContent::text("abc", "def"))
        );
        Ok(())
    }

    #[actix_rt::test]
    async fn convert_text_content_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;

        assert_eq!(
            NotificationContent::Text("text".to_string())
                .into_email(&api)
                .await?,
            EmailNotificationContent {
                subject: "[NO SUBJECT]".to_string(),
                text: "text".to_string(),
                html: None,
                attachments: None,
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn convert_email_content_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;

        assert_eq!(
            NotificationContent::Email(EmailNotificationContent::text("subject", "text"))
                .into_email(&api)
                .await?,
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: None,
                attachments: None,
            }
        );

        assert_eq!(
            NotificationContent::Email(EmailNotificationContent::html("subject", "text", "html"))
                .into_email(&api)
                .await?,
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
                attachments: None,
            }
        );

        assert_eq!(
            NotificationContent::Email(EmailNotificationContent::html_with_attachments(
                "subject",
                "text",
                "html",
                vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]
            ))
            .into_email(&api)
            .await?,
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
                attachments: Some(vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]),
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn convert_template_content_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let user = mock_user()?;
        let activation_code = "some-code";

        api.users().upsert(user.clone()).await?;
        api.db
            .upsert_user_data(
                InternalUserDataNamespace::AccountActivationToken,
                UserData::new(
                    user.id,
                    activation_code,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;

        assert_eq!(
            NotificationContent::Template(NotificationContentTemplate::AccountActivation(
                AccountActivationTemplate { user_id: user.id }
            ))
            .into_email(&api)
            .await?,
            EmailNotificationContent {
                subject: "Activate you Secutils.dev account".to_string(),
                text: "To activate your Secutils.dev account, please click the following link: http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev".to_string(),
                html: Some("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Activate your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n      body {\n          font-family: Arial, sans-serif;\n          background-color: #f1f1f1;\n          margin: 0;\n          padding: 0;\n      }\n      .container {\n          max-width: 600px;\n          margin: 0 auto;\n          background-color: #fff;\n          padding: 20px;\n          border-radius: 5px;\n          box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n      }\n      h1 {\n          font-size: 24px;\n          margin-top: 0;\n      }\n      p {\n          font-size: 16px;\n          line-height: 1.5;\n          margin-bottom: 20px;\n      }\n      .activate-link {\n          color: #fff;\n          background-color: #2196F3;\n          padding: 10px 20px;\n          text-decoration: none;\n          border-radius: 5px;\n      }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>Activate your Secutils.dev account</h1>\n  <p>Thanks for signing up! To activate your account, please click the link below:</p>\n  <a class=\"activate-link\" href=\"http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev\">Activate my account</a>\n  <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>\n  <p>http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev</p>\n  <p>If you have any trouble activating your account, please contact us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>.</p>\n</div>\n</body>\n</html>\n".to_string()),
                attachments: None,
            }
        );

        Ok(())
    }
}
