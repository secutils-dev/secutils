mod account_activation;
mod password_reset;
mod web_page_content_tracker_changes;
mod web_page_resources_tracker_changes;

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
    users::UserId,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContentTemplate {
    AccountActivation {
        user_id: UserId,
    },
    PasswordReset {
        user_id: UserId,
    },
    WebPageResourcesTrackerChanges {
        tracker_name: String,
        changes_count: usize,
    },
    WebPageContentTrackerChanges {
        tracker_name: String,
    },
}

impl NotificationContentTemplate {
    /// Compiles notification content template as an email.
    pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<EmailNotificationContent> {
        match self {
            NotificationContentTemplate::AccountActivation { user_id } => {
                account_activation::compile_to_email(api, *user_id).await
            }
            NotificationContentTemplate::PasswordReset { user_id } => {
                password_reset::compile_to_email(api, *user_id).await
            }
            NotificationContentTemplate::WebPageResourcesTrackerChanges {
                tracker_name,
                changes_count,
            } => {
                web_page_resources_tracker_changes::compile_to_email(
                    api,
                    tracker_name,
                    *changes_count,
                )
                .await
            }
            NotificationContentTemplate::WebPageContentTrackerChanges { tracker_name } => {
                web_page_content_tracker_changes::compile_to_email(api, tracker_name).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        notifications::NotificationContentTemplate,
        tests::{mock_api, mock_user},
        users::{InternalUserDataNamespace, UserData},
    };
    use insta::assert_debug_snapshot;
    use itertools::Itertools;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn can_compile_account_activation_template_to_email() -> anyhow::Result<()> {
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

        assert_debug_snapshot!(
             NotificationContentTemplate::AccountActivation { user_id: user.id }.compile_to_email(&api)
                .await?, @r###"
        EmailNotificationContent {
            subject: "Activate you Secutils.dev account",
            text: "To activate your Secutils.dev account, please click the following link: http://localhost:1234/activate?code=some-code&email=dev-1%40secutils.dev",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Activate your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .button-link {\n      color: #fff;\n      background-color: #2196F3;\n      padding: 10px 20px;\n      text-decoration: none;\n      border-radius: 5px;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>Activate your Secutils.dev account</h1>\n  <p>Thanks for signing up! To activate your account, please click the link below:</p>\n  <a class=\"button-link\" href=\"http://localhost:1234/activate?code=some-code&email=dev-1%40secutils.dev\">Activate my account</a>\n  <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>\n  <p>http://localhost:1234/activate?code=some-code&email=dev-1%40secutils.dev</p>\n  <p>If you have any trouble activating your account, please contact us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>.</p>\n</div>\n</body>\n</html>\n",
            ),
            attachments: None,
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_compile_password_reset_template_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;
        let user = mock_user()?;
        let reset_code = "some-code";

        api.users().upsert(user.clone()).await?;
        api.db
            .upsert_user_data(
                InternalUserDataNamespace::CredentialsResetToken,
                UserData::new(
                    user.id,
                    reset_code,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                ),
            )
            .await?;

        assert_debug_snapshot!(
             NotificationContentTemplate::PasswordReset { user_id: user.id }.compile_to_email(&api)
                .await?, @r###"
        EmailNotificationContent {
            subject: "Reset password for your Secutils.dev account",
            text: "To reset your Secutils.dev password, please click the following link: http://localhost:1234/reset_credentials?code=some-code&email=dev-1%40secutils.dev",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Reset password for your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .button-link {\n      color: #fff;\n      background-color: #2196F3;\n      padding: 10px 20px;\n      text-decoration: none;\n      border-radius: 5px;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>Reset password for your Secutils.dev account</h1>\n  <p>You recently requested to reset your password. To reset your password, please click the link below:</p>\n  <a class=\"button-link\" href=\"http://localhost:1234/reset_credentials?code=some-code&email=dev-1%40secutils.dev\">Reset your password</a>\n  <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>\n  <p>http://localhost:1234/reset_credentials?code=some-code&email=dev-1%40secutils.dev</p>\n  <p>If you did not request to reset your password, please ignore this email and your password will not be changed.</p>\n  <p>If you have any trouble resetting your password, please contact us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>.</p>\n</div>\n</body>\n</html>\n",
            ),
            attachments: None,
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_compile_resources_tracker_changes_template_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mut template = NotificationContentTemplate::WebPageResourcesTrackerChanges {
            tracker_name: "tracker".to_string(),
            changes_count: 10,
        }
        .compile_to_email(&api)
        .await?;
        template
            .attachments
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(|a| {
                a.content = a.content.len().to_be_bytes().iter().cloned().collect_vec();
            });

        assert_debug_snapshot!(template, @r###"
        EmailNotificationContent {
            subject: "Notification: \"tracker\" resources tracker detected 10 changes",
            text: "\"tracker\" resources tracker detected 10 changes. Visit http://localhost:1234/ws/web_scraping__resources to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" resources tracker detected 10 changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .button-link {\n      color: #fff;\n      background-color: #2196F3;\n      padding: 10px 20px;\n      text-decoration: none;\n      border-radius: 5px;\n    }\n  </style>\n  <style>\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" resources tracker detected 10 changes</h1>\n  <p>To learn more, visit the <b>Resources trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__resources\">Web Scraping → Resources trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__resources</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
            ),
            attachments: Some(
                [
                    EmailNotificationAttachment {
                        disposition: Inline(
                            "secutils-logo",
                        ),
                        content_type: "image/png",
                        content: [
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            15,
                            165,
                        ],
                    },
                ],
            ),
        }
        "###
        );

        Ok(())
    }

    #[tokio::test]
    async fn can_compile_content_tracker_changes_template_to_email() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mut template = NotificationContentTemplate::WebPageContentTrackerChanges {
            tracker_name: "tracker".to_string(),
        }
        .compile_to_email(&api)
        .await?;
        template
            .attachments
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(|a| {
                a.content = a.content.len().to_be_bytes().iter().cloned().collect_vec();
            });

        assert_debug_snapshot!(template, @r###"
        EmailNotificationContent {
            subject: "Notification: \"tracker\" content tracker detected changes",
            text: "\"tracker\" content tracker detected changes. Visit http://localhost:1234/ws/web_scraping__content to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" content tracker detected changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .button-link {\n      color: #fff;\n      background-color: #2196F3;\n      padding: 10px 20px;\n      text-decoration: none;\n      border-radius: 5px;\n    }\n  </style>\n  <style>\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" content tracker detected changes</h1>\n  <p>To learn more, visit the <b>Content trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__content\">Web Scraping → Content trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__content</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
            ),
            attachments: Some(
                [
                    EmailNotificationAttachment {
                        disposition: Inline(
                            "secutils-logo",
                        ),
                        content_type: "image/png",
                        content: [
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            15,
                            165,
                        ],
                    },
                ],
            ),
        }
        "###
        );

        Ok(())
    }
}
