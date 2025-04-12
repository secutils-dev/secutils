use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        EmailNotificationAttachment, EmailNotificationContent,
        notification_content_template::SECUTILS_LOGO_BYTES,
    },
};
use serde_json::json;

/// Compiles web page tracker content changes template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_name: &str,
    content: &Result<String, String>,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!("{}ws/web_scraping__content", api.config.public_url);
    let (subject, text, html) = match content {
        Ok(content) => (
            format!("[Secutils.dev] Change detected: \"{}\"", tracker_name),
            format!(
                "\"{}\" tracker detected content changes. Visit {} to learn more.",
                tracker_name, back_link
            ),
            api.templates.render(
                "web_page_content_tracker_changes_email",
                &json!({
                    "tracker_name": tracker_name,
                    "content": content,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?,
        ),
        Err(error_message) => (
            format!("[Secutils.dev] Check failed: \"{}\"", tracker_name),
            format!(
                "\"{}\" tracker failed to check for content changes due to the following error: {error_message}. Visit {} to learn more.",
                tracker_name, back_link
            ),
            api.templates.render(
                "web_page_content_tracker_changes_error_email",
                &json!({
                    "tracker_name": tracker_name,
                    "error_message": error_message,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?,
        ),
    };

    Ok(EmailNotificationContent::html_with_attachments(
        subject,
        text,
        html,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )],
    ))
}
