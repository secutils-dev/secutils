mod account_activation;
mod account_recovery;
mod api_tracker_changes;
mod notification_destination_verification;
mod page_tracker_changes;

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
    users::NotificationChannelKind,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SECUTILS_LOGO_BYTES: &[u8] =
    include_bytes!("../../assets/logo/secutils-logo-with-text.png");

/// Plain-text equivalent of the muted HTML `email-footer` block. Appended to product-mail text
/// bodies whenever an unsubscribe URL is in scope, mirroring the HTML footer rendered by the
/// Handlebars templates so MUAs that hide the HTML part still surface the opt-out.
pub(crate) fn plain_text_footer(unsubscribe_url: &str) -> String {
    format!(
        "\n\n---\nYou're receiving this email because Secutils.dev product notifications are enabled for this address. To unsubscribe, visit: {unsubscribe_url}"
    )
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContentTemplate {
    AccountActivation {
        flow_id: Uuid,
        code: String,
    },
    AccountRecovery {
        code: String,
    },
    PageTrackerChanges {
        tracker_id: Uuid,
        tracker_name: String,
        content: Result<String, String>,
        diff: Option<String>,
    },
    ApiTrackerChanges {
        tracker_id: Uuid,
        tracker_name: String,
        content: Result<String, String>,
        diff: Option<String>,
    },
    /// Verification challenge for an opt-in notification destination (e.g. a custom notification
    /// email). Body carries the 6-digit code only. Routed via `NotificationDestination::Email`,
    /// not `User`, so the proposed recipient never collides with the login email.
    NotificationDestinationVerification {
        kind: NotificationChannelKind,
        code: String,
    },
}

impl NotificationContentTemplate {
    /// Compiles notification content template as an email.
    ///
    /// `unsubscribe_url` is forwarded to product-mail templates (page/API tracker changes) so
    /// they can render a visible unsubscribe footer next to the RFC 8058 `List-Unsubscribe`
    /// header. Transactional templates (account activation/recovery, destination
    /// verification) intentionally ignore it: per RFC 8058 they are exempt from carrying
    /// unsubscribe affordances and must not invite the user to opt out of security-critical
    /// mail.
    pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
        unsubscribe_url: Option<&str>,
    ) -> anyhow::Result<EmailNotificationContent> {
        match self {
            NotificationContentTemplate::AccountActivation { code, flow_id } => {
                account_activation::compile_to_email(api, *flow_id, code).await
            }
            NotificationContentTemplate::AccountRecovery { code } => {
                account_recovery::compile_to_email(api, code).await
            }
            NotificationContentTemplate::PageTrackerChanges {
                tracker_id,
                tracker_name,
                content,
                diff,
            } => {
                page_tracker_changes::compile_to_email(
                    api,
                    *tracker_id,
                    tracker_name,
                    content,
                    diff.as_deref(),
                    unsubscribe_url,
                )
                .await
            }
            NotificationContentTemplate::ApiTrackerChanges {
                tracker_id,
                tracker_name,
                content,
                diff,
            } => {
                api_tracker_changes::compile_to_email(
                    api,
                    *tracker_id,
                    tracker_name,
                    content,
                    diff.as_deref(),
                    unsubscribe_url,
                )
                .await
            }
            NotificationContentTemplate::NotificationDestinationVerification { kind, code } => {
                notification_destination_verification::compile_to_email(api, *kind, code).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        notifications::NotificationContentTemplate, tests::mock_api, users::NotificationChannelKind,
    };
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
        .compile_to_email(&api, None)
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
            text: "To activate your Secutils.dev account, please use the following code: some-code. Alternatively, navigate to the following URL in your browser: https://secutils.dev/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Activate your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <p>Hi there,</p>\n  <p>Thanks for signing up! To activate your account, please click the button below:</p>\n  <a class=\"navigate-link\" href=\"https://secutils.dev/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001\">Activate my account</a>\n  <p>Alternatively, copy and paste the following URL into your browser:</p>\n  <p>https://secutils.dev/activate?code=some-code&flow=00000000-0000-0000-0000-000000000001</p>\n  <p>Or, simply copy and paste the following code into the account activation form:</p>\n  <p class=\"numeric-code\">some-code</p>\n  <p>If you have any trouble activating your account, please email us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>\n    or simply reply to this email.</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
        .compile_to_email(&api, None)
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
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Recover access to your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <p>Hi there,</p>\n  <p>You recently requested to recover access to your account. To do so, please copy and paste the following code into the account recovery form:</p>\n  <p class=\"numeric-code\">some-code</p>\n  <p>If you did not request to recover your account, please ignore this email.</p>\n  <p>If you have any trouble recovering your account, please email us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>\n    or simply reply to this email.</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
    async fn can_compile_page_tracker_changes_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::PageTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "tracker".to_string(),
            content: Ok("content".to_string()),
            diff: None,
        }
        .compile_to_email(&api, None)
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
            text: "\"tracker\" tracker detected changes. Visit https://secutils.dev/ws/web_scraping__page?q=00000000-0000-0000-0000-000000000001 to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker detected changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n  <style>\n    .diff-block {\n        font-family: 'Courier New', Courier, monospace;\n        font-size: 13px;\n        line-height: 1.4;\n        border: 1px solid #d0d7de;\n        border-radius: 6px;\n        overflow: auto;\n        margin-bottom: 20px;\n    }\n    .diff-block div {\n        padding: 1px 10px;\n        white-space: pre-wrap;\n        word-break: break-all;\n    }\n    .diff-add {\n        background-color: #e6ffec;\n        color: #1a7f37;\n    }\n    .diff-del {\n        background-color: #ffebe9;\n        color: #cf222e;\n    }\n    .diff-hunk {\n        background-color: #ddf4ff;\n        color: #0969da;\n        font-style: italic;\n    }\n    .diff-ctx {\n        background-color: #ffffff;\n        color: #1f2328;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker detected changes</h1>\n  <p>Current content: content</p>\n  <p>To learn more, visit the <b>Page trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"https://secutils.dev/ws/web_scraping__page?q&#x3D;00000000-0000-0000-0000-000000000001\">Web Scraping → Page trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>https://secutils.dev/ws/web_scraping__page?q&#x3D;00000000-0000-0000-0000-000000000001</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
    async fn can_compile_page_tracker_changes_with_diff_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let long_content = "a".repeat(300);

        let mut template = NotificationContentTemplate::PageTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "tracker".to_string(),
            content: Ok(long_content),
            diff: Some("@@ -1 +1 @@\n-old line\n+new line\n".to_string()),
        }
        .compile_to_email(&api, None)
        .await?;
        template
            .attachments
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(|a| {
                a.content = a.content.len().to_be_bytes().iter().cloned().collect_vec();
            });

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("<div class=\"diff-block\">"),
            "Should contain diff block"
        );
        assert!(
            html.contains("<div class=\"diff-hunk\">@@ -1 +1 @@</div>"),
            "Should contain hunk header"
        );
        assert!(
            html.contains("<div class=\"diff-del\">-old line</div>"),
            "Should contain deletion line"
        );
        assert!(
            html.contains("<div class=\"diff-add\">+new line</div>"),
            "Should contain addition line"
        );
        assert!(
            !html.contains("Current content:"),
            "Should NOT contain full content when diff is shown"
        );
        assert!(
            html.contains("Here's what changed:"),
            "Should contain diff intro text"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_compile_page_tracker_short_content_with_diff_shows_content(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let template = NotificationContentTemplate::PageTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "tracker".to_string(),
            content: Ok("short".to_string()),
            diff: Some("@@ -1 +1 @@\n-old\n+short\n".to_string()),
        }
        .compile_to_email(&api, None)
        .await?;

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("Current content: short"),
            "Short content should show full content"
        );
        assert!(
            !html.contains("<div class=\"diff-block\">"),
            "Short content should NOT show diff"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_compile_page_tracker_changes_error_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::PageTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "tracker".to_string(),
            content: Err("Something went wrong".to_string()),
            diff: None,
        }
        .compile_to_email(&api, None)
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
            text: "\"tracker\" tracker failed to check for changes due to the following error: Something went wrong. Visit https://secutils.dev/ws/web_scraping__page?q=00000000-0000-0000-0000-000000000001 to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"tracker\" tracker failed to check for changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"tracker\" tracker failed to check for changes</h1>\n  <p>There was an error while checking for changes: <b>Something went wrong</b>.</p>\n  <p>To check the tracker configuration and re-try, visit the <b>Page trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"https://secutils.dev/ws/web_scraping__page?q&#x3D;00000000-0000-0000-0000-000000000001\">Web Scraping → Page trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>https://secutils.dev/ws/web_scraping__page?q&#x3D;00000000-0000-0000-0000-000000000001</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
    async fn can_compile_api_tracker_changes_template_to_email(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::ApiTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "api-tracker".to_string(),
            content: Ok("content".to_string()),
            diff: None,
        }
        .compile_to_email(&api, None)
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
            subject: "[Secutils.dev] Change detected: \"api-tracker\"",
            text: "\"api-tracker\" API tracker detected changes. Visit https://secutils.dev/ws/web_scraping__api?q=00000000-0000-0000-0000-000000000001 to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"api-tracker\" API tracker detected changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n  <style>\n    .diff-block {\n        font-family: 'Courier New', Courier, monospace;\n        font-size: 13px;\n        line-height: 1.4;\n        border: 1px solid #d0d7de;\n        border-radius: 6px;\n        overflow: auto;\n        margin-bottom: 20px;\n    }\n    .diff-block div {\n        padding: 1px 10px;\n        white-space: pre-wrap;\n        word-break: break-all;\n    }\n    .diff-add {\n        background-color: #e6ffec;\n        color: #1a7f37;\n    }\n    .diff-del {\n        background-color: #ffebe9;\n        color: #cf222e;\n    }\n    .diff-hunk {\n        background-color: #ddf4ff;\n        color: #0969da;\n        font-style: italic;\n    }\n    .diff-ctx {\n        background-color: #ffffff;\n        color: #1f2328;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"api-tracker\" API tracker detected changes</h1>\n  <p>Current content: content</p>\n  <p>To learn more, visit the <b>API trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"https://secutils.dev/ws/web_scraping__api?q&#x3D;00000000-0000-0000-0000-000000000001\">Web Scraping → API trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>https://secutils.dev/ws/web_scraping__api?q&#x3D;00000000-0000-0000-0000-000000000001</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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
    async fn can_compile_api_tracker_changes_with_diff_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;
        let long_content = "a".repeat(300);

        let mut template = NotificationContentTemplate::ApiTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "api-tracker".to_string(),
            content: Ok(long_content),
            diff: Some("@@ -1 +1 @@\n-old line\n+new line\n".to_string()),
        }
        .compile_to_email(&api, None)
        .await?;
        template
            .attachments
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(|a| {
                a.content = a.content.len().to_be_bytes().iter().cloned().collect_vec();
            });

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("<div class=\"diff-block\">"),
            "Should contain diff block"
        );
        assert!(
            html.contains("<div class=\"diff-hunk\">@@ -1 +1 @@</div>"),
            "Should contain hunk header"
        );
        assert!(
            html.contains("<div class=\"diff-del\">-old line</div>"),
            "Should contain deletion line"
        );
        assert!(
            html.contains("<div class=\"diff-add\">+new line</div>"),
            "Should contain addition line"
        );
        assert!(
            !html.contains("Current content:"),
            "Should NOT contain full content when diff is shown"
        );
        assert!(
            html.contains("Here's what changed:"),
            "Should contain diff intro text"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_compile_api_tracker_short_content_with_diff_shows_content(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let template = NotificationContentTemplate::ApiTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "api-tracker".to_string(),
            content: Ok("short".to_string()),
            diff: Some("@@ -1 +1 @@\n-old\n+short\n".to_string()),
        }
        .compile_to_email(&api, None)
        .await?;

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("Current content: short"),
            "Short content should show full content"
        );
        assert!(
            !html.contains("<div class=\"diff-block\">"),
            "Short content should NOT show diff"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn can_compile_notification_destination_verification_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::NotificationDestinationVerification {
            kind: NotificationChannelKind::Email,
            code: "ABC123".to_string(),
        }
        .compile_to_email(&api, None)
        .await?;
        template
            .attachments
            .as_mut()
            .unwrap()
            .iter_mut()
            .for_each(|a| {
                a.content = a.content.len().to_be_bytes().iter().cloned().collect_vec();
            });

        assert_eq!(
            template.subject,
            "Verify your Secutils.dev notification email"
        );
        assert!(
            template
                .text
                .contains("To start receiving Secutils.dev notifications at this address"),
            "plain text should explain the flow"
        );
        assert!(
            template.text.contains("ABC123"),
            "plain text should embed the code"
        );
        assert!(
            template.text.contains("expires in 15 minutes"),
            "plain text should mention TTL"
        );

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("<title>Verify your Secutils.dev notification email</title>"),
            "HTML should set the verification title"
        );
        assert!(
            html.contains(r#"<p class="numeric-code">ABC123</p>"#),
            "HTML should render the code in the numeric-code box"
        );
        assert!(
            html.contains("https://secutils.dev/"),
            "HTML should embed the configured public URL as the home link"
        );
        assert!(
            html.contains(r#"<img src="cid:secutils-logo""#),
            "HTML should reference the inline secutils-logo attachment"
        );

        // The single inline attachment is the secutils-logo, content patched to a
        // length-encoded marker upstream — confirm the disposition shape is unchanged.
        assert_debug_snapshot!(template.attachments, @r###"
        Some(
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
        )
        "###);

        Ok(())
    }

    #[sqlx::test]
    async fn can_compile_api_tracker_changes_error_template_to_email(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let mut template = NotificationContentTemplate::ApiTrackerChanges {
            tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
            tracker_name: "api-tracker".to_string(),
            content: Err("Something went wrong".to_string()),
            diff: None,
        }
        .compile_to_email(&api, None)
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
            subject: "[Secutils.dev] Check failed: \"api-tracker\"",
            text: "\"api-tracker\" API tracker failed to check for changes due to the following error: Something went wrong. Visit https://secutils.dev/ws/web_scraping__api?q=00000000-0000-0000-0000-000000000001 to learn more.",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>\"api-tracker\" API tracker failed to check for changes</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .navigate-link {\n      display: block;\n      width: 250px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      text-decoration: none;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .numeric-code {\n      display: block;\n      width: 100px;\n      margin: auto;\n      padding: 10px 20px;\n      text-align: center;\n      color: #5e1d3f;\n      background-color: #fed047;\n      border-radius: 5px;\n      font-weight: bold;\n    }\n    .email-footer {\n      margin-top: 24px;\n      padding-top: 16px;\n      border-top: 1px solid #e5e7eb;\n      text-align: center;\n    }\n    .email-footer p {\n      font-size: 12px;\n      line-height: 1.5;\n      color: #6b7280;\n      margin: 0 0 4px 0;\n    }\n    .email-footer a {\n      color: #4b5563;\n      text-decoration: underline;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>\"api-tracker\" API tracker failed to check for changes</h1>\n  <p>There was an error while checking for changes: <b>Something went wrong</b>.</p>\n  <p>To check the tracker configuration and re-try, visit the <b>API trackers</b> page:</p>\n  <a class=\"navigate-link\" href=\"https://secutils.dev/ws/web_scraping__api?q&#x3D;00000000-0000-0000-0000-000000000001\">Web Scraping → API trackers</a>\n  <p>If the button above doesn't work, you can navigate to the following URL directly: </p>\n  <p>https://secutils.dev/ws/web_scraping__api?q&#x3D;00000000-0000-0000-0000-000000000001</p>\n  <a href=\"https://secutils.dev/\"><img src=\"cid:secutils-logo\" alt=\"Secutils.dev logo\" width=\"89\" height=\"14\" /></a>\n</div>\n</body>\n</html>\n",
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

    /// Tests covering the unsubscribe footer rendering contract — the muted body block
    /// only appears for product-mail templates (page/API tracker changes) when an
    /// unsubscribe URL is threaded through, and is absent for transactional templates
    /// (account activation/recovery, destination verification) regardless.
    mod unsubscribe_footer {
        use super::*;
        use crate::users::NotificationChannelKind;

        const UNSUBSCRIBE_URL: &str =
            "https://secutils.dev/api/notifications/unsubscribe?token=tok-123";

        fn assert_footer_present(html: &str, text: &str) {
            assert!(
                html.contains(r#"<div class="email-footer">"#),
                "HTML should render the muted footer wrapper"
            );
            // Handlebars escapes `=` to `&#x3D;` in HTML attributes, so we assert against
            // the URL prefix and token components separately rather than the raw URL.
            assert!(
                html.contains(r#"<a href="https://secutils.dev/api/notifications/unsubscribe"#),
                "HTML should embed the unsubscribe link target"
            );
            assert!(
                html.contains("token&#x3D;tok-123"),
                "HTML should carry the token in the (HTML-escaped) URL query string"
            );
            assert!(
                html.contains(">Unsubscribe</a>"),
                "HTML should label the link 'Unsubscribe'"
            );
            assert!(
                html.contains(
                    "You're receiving this email because Secutils.dev product notifications are enabled for this address."
                ),
                "HTML should explain why the email was sent"
            );
            // Plain-text body is not HTML-escaped, so we assert the raw URL.
            assert!(
                text.contains(UNSUBSCRIBE_URL),
                "plain text body should contain the unsubscribe URL verbatim"
            );
            assert!(
                text.contains("To unsubscribe, visit:"),
                "plain text footer should include the opt-out copy"
            );
        }

        fn assert_footer_absent(html: &str, text: &str) {
            assert!(
                !html.contains(r#"<div class="email-footer">"#),
                "HTML should not render the footer block when no unsubscribe URL is set"
            );
            assert!(
                !html.contains(">Unsubscribe<"),
                "HTML should not embed an Unsubscribe link"
            );
            assert!(
                !text.contains("To unsubscribe, visit:"),
                "plain text footer block must not appear without an URL"
            );
        }

        #[sqlx::test]
        async fn page_tracker_changes_renders_footer_when_url_present(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::PageTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "tracker".to_string(),
                content: Ok("content".to_string()),
                diff: None,
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_present(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn page_tracker_changes_omits_footer_when_url_absent(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::PageTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "tracker".to_string(),
                content: Ok("content".to_string()),
                diff: None,
            }
            .compile_to_email(&api, None)
            .await?;

            assert_footer_absent(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn page_tracker_changes_error_renders_footer_when_url_present(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::PageTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "tracker".to_string(),
                content: Err("boom".to_string()),
                diff: None,
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_present(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn api_tracker_changes_renders_footer_when_url_present(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::ApiTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "api-tracker".to_string(),
                content: Ok("content".to_string()),
                diff: None,
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_present(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn api_tracker_changes_omits_footer_when_url_absent(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::ApiTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "api-tracker".to_string(),
                content: Ok("content".to_string()),
                diff: None,
            }
            .compile_to_email(&api, None)
            .await?;

            assert_footer_absent(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn api_tracker_changes_error_renders_footer_when_url_present(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::ApiTrackerChanges {
                tracker_id: uuid!("00000000-0000-0000-0000-000000000001"),
                tracker_name: "api-tracker".to_string(),
                content: Err("boom".to_string()),
                diff: None,
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_present(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        // Transactional templates — even when an unsubscribe URL is supplied (which never
        // happens in production, but the contract is enforced here as a belt-and-braces
        // assertion) the rendered email must not invite the user to opt out of mail that
        // is exempt under RFC 8058.

        #[sqlx::test]
        async fn account_activation_ignores_unsubscribe_url(pool: PgPool) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::AccountActivation {
                flow_id: uuid!("00000000-0000-0000-0000-000000000001"),
                code: "code".to_string(),
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_absent(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn account_recovery_ignores_unsubscribe_url(pool: PgPool) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::AccountRecovery {
                code: "code".to_string(),
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_absent(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }

        #[sqlx::test]
        async fn notification_destination_verification_ignores_unsubscribe_url(
            pool: PgPool,
        ) -> anyhow::Result<()> {
            let api = mock_api(pool).await?;
            let template = NotificationContentTemplate::NotificationDestinationVerification {
                kind: NotificationChannelKind::Email,
                code: "ABC123".to_string(),
            }
            .compile_to_email(&api, Some(UNSUBSCRIBE_URL))
            .await?;

            assert_footer_absent(template.html.as_deref().unwrap(), &template.text);
            Ok(())
        }
    }
}
