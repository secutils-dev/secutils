use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        notification_content_template::SECUTILS_LOGO_BYTES, EmailNotificationAttachment,
        EmailNotificationContent,
    },
};
use serde_json::json;

/// Compiles account recovery template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    code: &str,
) -> anyhow::Result<EmailNotificationContent> {
    let encoded_code = urlencoding::encode(code);
    Ok(EmailNotificationContent::html_with_attachments(
        "Recover access to your Secutils.dev account",
        format!("To recover your Secutils.dev account, please use the following code in the account recovery form: {encoded_code}."),
        api.templates.render(
            "account_recovery_email",
            &json!({ "encoded_recovery_code": encoded_code, "home_link": api.config.public_url.as_str() })
        )?,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )]
    ))
}
