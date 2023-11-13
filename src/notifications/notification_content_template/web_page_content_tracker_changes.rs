use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{EmailNotificationAttachment, EmailNotificationContent},
};
use serde_json::json;

pub const NOTIFICATION_LOGO_BYTES: &[u8] =
    include_bytes!("../../../assets/logo/secutils-logo-with-text.png");

/// Compiles web page tracker content changes template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_name: &str,
    content: &str,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!("{}ws/web_scraping__content", api.config.public_url);
    Ok(EmailNotificationContent::html_with_attachments(
        format!(
            "Notification: \"{}\" content tracker detected changes",
            tracker_name
        ),
        format!(
            "\"{}\" content tracker detected changes: \"{}\". Visit {} to learn more.",
            tracker_name, back_link, content
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
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            NOTIFICATION_LOGO_BYTES.to_vec(),
        )],
    ))
}
