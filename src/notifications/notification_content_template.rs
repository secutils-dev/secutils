mod templates;

use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
};
use serde::{Deserialize, Serialize};

pub use self::templates::AccountActivationTemplate;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NotificationContentTemplate {
    AccountActivation(AccountActivationTemplate),
}

impl NotificationContentTemplate {
    /// Compiles notification content template as an email.
    pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<EmailNotificationContent> {
        match self {
            NotificationContentTemplate::AccountActivation(template) => {
                template.compile_to_email(api).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        notifications::{
            AccountActivationTemplate, EmailNotificationContent, NotificationContentTemplate,
        },
        tests::{mock_api, mock_user},
        users::{InternalUserDataNamespace, UserData},
    };
    use time::OffsetDateTime;

    #[actix_rt::test]
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

        assert_eq!(
            NotificationContentTemplate::AccountActivation(AccountActivationTemplate { user_id: user.id }).compile_to_email(&api)
                .await?,
            EmailNotificationContent {
                subject: "Activate you Secutils.dev account".to_string(),
                text: "To activate your Secutils.dev account, please click the following link: http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev".to_string(),
                html: Some("\n<!DOCTYPE html>\n<html>\n  <head>\n    <title>Activate your Secutils.dev account</title>\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n    <style>\n      body {\n        font-family: Arial, sans-serif;\n        background-color: #f1f1f1;\n        margin: 0;\n        padding: 0;\n      }\n      .container {\n        max-width: 600px;\n        margin: 0 auto;\n        background-color: #fff;\n        padding: 20px;\n        border-radius: 5px;\n        box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);\n      }\n      h1 {\n        font-size: 24px;\n        margin-top: 0;\n      }\n      p {\n        font-size: 16px;\n        line-height: 1.5;\n        margin-bottom: 20px;\n      }\n      .activate-link {\n        color: #fff;\n        background-color: #2196F3;\n        padding: 10px 20px;\n        text-decoration: none;\n        border-radius: 5px;\n      }\n    </style>\n  </head>\n  <body>\n    <div class=\"container\">\n      <h1>Activate your Secutils.dev account</h1>\n      <p>Thanks for signing up! To activate your account, please click the link below:</p>\n      <a class=\"activate-link\" href=\"http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev\">Activate my account</a>\n      <p>If the button above doesn't work, you can also copy and paste the following URL into your browser:</p>\n      <p>http://localhost:1234/activate?code=some-code&email=dev%40secutils.dev</p>\n      <p>If you have any trouble activating your account, please contact us at <a href = \"mailto: contact@secutils.dev\">contact@secutils.dev</a>.</p>\n    </div>\n  </body>\n</html>".to_string()),
                attachments: None,
            }
        );

        Ok(())
    }
}
