mod account_activation;
mod account_recovery;
mod web_page_content_tracker_changes;
mod web_page_resources_tracker_changes;

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SECUTILS_LOGO_BYTES: &[u8] =
    include_bytes!("../../assets/logo/secutils-logo-with-text.png");

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContentTemplate {
    AccountActivation {
        flow_id: Uuid,
        code: String,
    },
    AccountRecovery {
        code: String,
    },
    WebPageResourcesTrackerChanges {
        tracker_name: String,
        content: Result<usize, String>,
    },
    WebPageContentTrackerChanges {
        tracker_name: String,
        content: Result<String, String>,
    },
}

impl NotificationContentTemplate {
    /// Compiles notification content template as an email.
    pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<EmailNotificationContent> {
        match self {
            NotificationContentTemplate::AccountActivation { code, flow_id } => {
                account_activation::compile_to_email(api, *flow_id, code).await
            }
            NotificationContentTemplate::AccountRecovery { code } => {
                account_recovery::compile_to_email(api, code).await
            }
            NotificationContentTemplate::WebPageResourcesTrackerChanges {
                tracker_name,
                content,
            } => {
                web_page_resources_tracker_changes::compile_to_email(api, tracker_name, content)
                    .await
            }
            NotificationContentTemplate::WebPageContentTrackerChanges {
                tracker_name,
                content,
            } => {
                web_page_content_tracker_changes::compile_to_email(api, tracker_name, content).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{notifications::NotificationContentTemplate, tests::mock_api};
    use insta::assert_debug_snapshot;
    use itertools::Itertools;
    use sqlx::PgPool;
    use uuid::uuid;

    #[sqlx::test]
    async fn can_compile_account_activation_template_to_email(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let activation_code = "some-code";

        let mut template = NotificationContentTemplate::AccountActivation {
            flow_id: uuid!("00000000-0000-0000-0000-000000000001"),
            code: activation_code.to_string(),
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

        assert_debug_snapshot!(
             template, @r###"
        EmailNotificationContent {
            subject: "Activate your Secutils.dev account",
            text: "To activate your Secutils.dev account, please use the following code: some-code. Alternatively, navigate to the following URL in your browser: http://localhost:1234/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Activate your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <p>Hi there,</p>\n  <p>Thanks for signing up! To activate your account, please click the button below:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001\">Activate my account</a>\n  <p>Alternatively, copy and paste the following URL into your browser:</p>\n  <p>http://localhost:1234/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001</p>\n  <p>Or, simply copy and paste the following code into the account activation form:</p>\n  <p class=\"numeric-code\">some-code</p>\n  <p>If you have any trouble activating your account, please email us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>\n    or simply reply to this email.</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    #[sqlx::test]
    async fn can_compile_password_reset_template_to_email(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let recovery_code = "some-code";

        let mut template = NotificationContentTemplate::AccountRecovery {
            code: recovery_code.to_string(),
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

        assert_debug_snapshot!(
             template, @r###"
        EmailNotificationContent {
            subject: "Recover access to your Secutils.dev account",
            text: "To recover your Secutils.dev account, please use the following code in the account recovery form: some-code.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Recover access to your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <p>Hi there,</p>\n  <p>You recently requested to recover access to your account. To do so, please copy and paste the following code into the account recovery form:</p>\n  <p class=\"numeric-code\">some-code</p>\n  <p>If you did not request to recover your account, please ignore this email.</p>\n  <p>If you have any trouble recovering your account, please email us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>\n    or simply reply to this email.</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    #[sqlx::test]
    async fn can_compile_resources_tracker_changes_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::WebPageResourcesTrackerChanges {
            tracker_name: "tracker".to_string(),
            content: Ok(10),
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
            subject: "[Secutils.dev] Change detected: \"tracker\"",
            text: "\"tracker\" tracker detected 10 changes in resources. Visit http://localhost:1234/ws/web_scraping__resources to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker detected 10 changes in resources</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker detected 10 changes in resources</h1>\n  <p>To learn more, visit the <b>Resources trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__resources\">Web Scraping → Resources trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__resources</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    #[sqlx::test]
    async fn can_compile_resources_tracker_changes_error_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::WebPageResourcesTrackerChanges {
            tracker_name: "tracker".to_string(),
            content: Err("Something went wrong".to_string()),
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
            subject: "[Secutils.dev] Check failed: \"tracker\"",
            text: "\"tracker\" tracker failed to check for changes in resources due to the following error: Something went wrong. Visit http://localhost:1234/ws/web_scraping__resources to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker failed to check for changes in resources</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker failed to check for changes in resources</h1>\n  <p>There was an error while checking resources: <b>Something went wrong</b>.</p>\n  <p>To check the tracker configuration and re-try, visit the <b>Resources trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__resources\">Web Scraping → Resources trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__resources</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    #[sqlx::test]
    async fn can_compile_content_tracker_changes_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::WebPageContentTrackerChanges {
            tracker_name: "tracker".to_string(),
            content: Ok("content".to_string()),
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
            subject: "[Secutils.dev] Change detected: \"tracker\"",
            text: "\"tracker\" tracker detected content changes. Visit http://localhost:1234/ws/web_scraping__content to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker detected content changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker detected content changes</h1>\n  <p>Current content: content</p>\n  <p>To learn more, visit the <b>Content trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__content\">Web Scraping → Content trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__content</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    #[sqlx::test]
    async fn can_compile_content_tracker_changes_error_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::WebPageContentTrackerChanges {
            tracker_name: "tracker".to_string(),
            content: Err("Something went wrong".to_string()),
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
            subject: "[Secutils.dev] Check failed: \"tracker\"",
            text: "\"tracker\" tracker failed to check for content changes due to the following error: Something went wrong. Visit http://localhost:1234/ws/web_scraping__content to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker failed to check for content changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker failed to check for content changes</h1>\n  <p>There was an error while checking content: <b>Something went wrong</b>.</p>\n  <p>To check the tracker configuration and re-try, visit the <b>Content trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"http://localhost:1234/ws/web_scraping__content\">Web Scraping → Content trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>http://localhost:1234/ws/web_scraping__content</p>\n  <a href=\"http://localhost:1234/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
