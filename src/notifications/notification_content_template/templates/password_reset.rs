use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
    users::{InternalUserDataNamespace, UserId},
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct PasswordResetTemplate {
    pub user_id: UserId,
}

impl PasswordResetTemplate {
    /// Compiles account activation template as an email.
    pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<EmailNotificationContent> {
        let users_api = api.users();
        let reset_code = users_api
            .get_data::<String>(self.user_id, InternalUserDataNamespace::CredentialsResetToken)
            .await?
            .with_context(|| {
                format!("User ({}) doesn't have assigned activation code. Account activation isn't possible.", *self.user_id)
            })?;
        let Some(user) = users_api.get(self.user_id).await? else {
            anyhow::bail!("User ({}) is not found.", *self.user_id);
        };

        // For now we send email tailored for the password reset, but eventually we can allow user
        // to reset passkey as well.
        let encoded_reset_link = format!(
            "{}reset_credentials?code={}&email={}",
            api.config.public_url.as_str(),
            urlencoding::encode(&reset_code.value),
            urlencoding::encode(&user.email)
        );

        Ok(EmailNotificationContent::html(
            "Reset password for your Secutils.dev account",
            format!("To reset your Secutils.dev password, please click the following link: {encoded_reset_link}"),
            api.templates.render("password_reset_email", &json!({ "encoded_reset_link": encoded_reset_link }))?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        notifications::PasswordResetTemplate,
        tests::{mock_api, mock_user},
        users::{InternalUserDataNamespace, UserData},
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn can_compile_to_email() -> anyhow::Result<()> {
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
            PasswordResetTemplate { user_id: user.id }.compile_to_email(&api).await?, @r###"
        EmailNotificationContent {
            subject: "Reset password for your Secutils.dev account",
            text: "To reset your Secutils.dev password, please click the following link: http://localhost:1234/reset_credentials?code=some-code&email=dev%40secutils.dev",
            html: Some(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Reset password for your Secutils.dev account</title>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <style>\n    body {\n      font-family: Arial, sans-serif;\n      background-color: #f1f1f1;\n      margin: 0;\n      padding: 0;\n    }\n    .container {\n      max-width: 600px;\n      margin: 0 auto;\n      background-color: #fff;\n      padding: 20px;\n      border-radius: 5px;\n      box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n    }\n    h1 {\n      font-size: 24px;\n      margin-top: 0;\n    }\n    p {\n      font-size: 16px;\n      line-height: 1.5;\n      margin-bottom: 20px;\n    }\n    .button-link {\n      color: #fff;\n      background-color: #2196F3;\n      padding: 10px 20px;\n      text-decoration: none;\n      border-radius: 5px;\n    }\n  </style>\n</head>\n<body>\n<div class=\"container\">\n  <h1>Reset password for your Secutils.dev account</h1>\n  <p>You recently requested to reset your password. To reset your password, please click the link below:</p>\n  <a class=\"button-link\" href=\"http://localhost:1234/reset_credentials?code=some-code&email=dev%40secutils.dev\">Reset your password</a>\n  <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>\n  <p>http://localhost:1234/reset_credentials?code=some-code&email=dev%40secutils.dev</p>\n  <p>If you did not request to reset your password, please ignore this email and your password will not be changed.</p>\n  <p>If you have any trouble resetting your password, please contact us at <a href=\"mailto: contact@secutils.dev\">contact@secutils.dev</a>.</p>\n</div>\n</body>\n</html>\n",
            ),
            attachments: None,
        }
        "###
        );

        Ok(())
    }
}
