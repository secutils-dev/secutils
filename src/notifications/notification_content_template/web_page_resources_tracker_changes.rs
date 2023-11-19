use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{EmailNotificationAttachment, EmailNotificationContent},
};
use serde_json::json;

pub const NOTIFICATION_LOGO_BYTES: &[u8] =
    include_bytes!("../../../assets/logo/secutils-logo-with-text.png");

/// Compiles web page tracker resources changes template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_name: &str,
    changes_count: usize,
    error_message: Option<&str>,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!("{}ws/web_scraping__resources", api.config.public_url);

    let (subject, text, html) = if let Some(error_message) = error_message {
        (
            format!("[Secutils.dev] Check failed: \"{}\"", tracker_name),
            format!(
                "\"{}\" tracker failed to check for changes in resources due to the following error: {error_message}. Visit {} to learn more.",
                tracker_name, back_link
            ),
            api.templates.render(
                "web_page_resources_tracker_changes_error_email",
                &json!({
                    "tracker_name": tracker_name,
                    "error_message": error_message,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?
        )
    } else {
        (
            format!("[Secutils.dev] Change detected: \"{}\"", tracker_name),
            format!(
                "\"{}\" tracker detected {} changes in resources. Visit {} to learn more.",
                tracker_name, changes_count, back_link
            ),
            api.templates.render(
                "web_page_resources_tracker_changes_email",
                &json!({
                    "tracker_name": tracker_name,
                    "changes_count": changes_count,
                    "back_link": back_link,
                    "home_link": api.config.public_url.as_str(),
                }),
            )?,
        )
    };

    Ok(EmailNotificationContent::html_with_attachments(
        subject,
        text,
        html,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            NOTIFICATION_LOGO_BYTES.to_vec(),
        )],
    ))
}
