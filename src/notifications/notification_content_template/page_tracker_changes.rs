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

/// Compiles page tracker changes template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_id: Uuid,
    tracker_name: &str,
    content: &Result<String, String>,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!(
        "{}ws/web_scraping__page?q={}",
        api.config.public_url, tracker_id
    );
    let (subject, text, html) = match content {
        Ok(content) => (
            format!("[Secutils.dev] Change detected: \"{tracker_name}\""),
            format!(
                "\"{tracker_name}\" tracker detected changes. Visit {back_link} to learn more.",
            ),
            api.templates.render(
                "page_tracker_changes_email",
                &json!({
                    "tracker_name": tracker_name,
                    "content": content,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?,
        ),
        Err(error_message) => (
            format!("[Secutils.dev] Check failed: \"{tracker_name}\""),
            format!(
                "\"{tracker_name}\" tracker failed to check for changes due to the following error: {error_message}. Visit {back_link} to learn more."
            ),
            api.templates.render(
                "page_tracker_changes_error_email",
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
