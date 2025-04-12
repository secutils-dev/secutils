use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        EmailNotificationAttachment, EmailNotificationContent,
        notification_content_template::SECUTILS_LOGO_BYTES,
    },
};
use serde_json::json;
use uuid::Uuid;

/// Compiles account activation template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    flow_id: Uuid,
    code: &str,
) -> anyhow::Result<EmailNotificationContent> {
    if flow_id.is_nil() || code.is_empty() {
        anyhow::bail!(
            "Flow ID and code must be provided, but received code `{code}` and flow ID `{flow_id}`."
        );
    }

    let encoded_code = urlencoding::encode(code);
    let encoded_activation_link = format!(
        "{}activate?code={}&flow={}",
        api.config.public_url.as_str(),
        encoded_code,
        urlencoding::encode(&flow_id.as_hyphenated().to_string())
    );

    Ok(EmailNotificationContent::html_with_attachments(
        "Activate your Secutils.dev account",
        format!(
            "To activate your Secutils.dev account, please use the following code: {encoded_code}. Alternatively, navigate to the following URL in your browser: {encoded_activation_link}"
        ),
        api.templates.render(
            "account_activation_email",
            &json!({
                "encoded_activation_link": encoded_activation_link,
                "encoded_activation_code": encoded_code,
                "home_link": api.config.public_url.as_str()
            }),
        )?,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )],
    ))
}
