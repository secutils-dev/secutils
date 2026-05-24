use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        EmailNotificationAttachment, EmailNotificationContent,
        notification_content_template::SECUTILS_LOGO_BYTES,
    },
    users::NotificationChannelKind,
};
use serde_json::json;

/// Renders the "prove control of this destination" email used by the in-app notification email
/// verification flow. Today only `kind = Email` is wired up.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    kind: NotificationChannelKind,
    code: &str,
) -> anyhow::Result<EmailNotificationContent> {
    if code.is_empty() {
        anyhow::bail!("Verification code must be provided.");
    }

    match kind {
        NotificationChannelKind::Email => {
            let encoded_code = urlencoding::encode(code);
            Ok(EmailNotificationContent::html_with_attachments(
                "Verify your Secutils.dev notification email",
                format!(
                    "To start receiving Secutils.dev notifications at this address, please use the following verification code in the Settings page: {encoded_code}. The code expires in 15 minutes."
                ),
                api.templates.render(
                    "notification_destination_verification_email",
                    &json!({
                        "encoded_verification_code": encoded_code,
                        "home_link": api.config.public_url.as_str(),
                    }),
                )?,
                vec![EmailNotificationAttachment::inline(
                    "secutils-logo",
                    "image/png",
                    SECUTILS_LOGO_BYTES.to_vec(),
                )],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock_api;
    use sqlx::PgPool;

    #[sqlx::test]
    async fn rejects_empty_verification_code(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let result = compile_to_email(&api, NotificationChannelKind::Email, "").await;

        let err = result.expect_err("empty code should be rejected");
        assert!(
            err.to_string()
                .contains("Verification code must be provided"),
            "unexpected error: {err}"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn url_encodes_code_in_rendered_email(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        // Pick characters that round-trip differently raw vs URL-encoded so the assertion is
        // unambiguous: `+` -> `%2B`, `/` -> `%2F`, `=` -> `%3D`.
        let template = compile_to_email(&api, NotificationChannelKind::Email, "ab+cd/ef=").await?;

        let html = template.html.as_deref().unwrap();
        assert!(
            html.contains("ab%2Bcd%2Fef%3D"),
            "HTML should embed the URL-encoded code"
        );
        assert!(
            !html.contains("ab+cd/ef="),
            "HTML must not leak the raw code with reserved characters"
        );
        assert!(
            template.text.contains("ab%2Bcd%2Fef%3D"),
            "plain text should embed the URL-encoded code"
        );
        assert!(
            !template.text.contains("ab+cd/ef="),
            "plain text must not leak the raw code with reserved characters"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn embeds_configured_home_link(pool: PgPool) -> anyhow::Result<()> {
        let api = mock_api(pool).await?;

        let template = compile_to_email(&api, NotificationChannelKind::Email, "code").await?;

        let html = template.html.as_deref().unwrap();
        let home_link = api.config.public_url.as_str();
        assert!(
            html.contains(&format!(r#"<a href="{home_link}""#)),
            "HTML should anchor the secutils-logo to the configured public URL"
        );

        Ok(())
    }
}
