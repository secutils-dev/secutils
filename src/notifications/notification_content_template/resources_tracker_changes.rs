use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{EmailNotificationAttachment, EmailNotificationContent},
};
use serde_json::json;

pub const NOTIFICATION_LOGO_BYTES: &[u8] =
    include_bytes!("../../../assets/logo/secutils-logo-with-text.png");

/// Compiles account activation template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    tracker_name: &str,
    changes_count: usize,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!("{}ws/web_scraping__resources", api.config.public_url);
    Ok(EmailNotificationContent::html_with_attachments(
        format!(
            "Notification: \"{}\" resources tracker detected {} changes",
            tracker_name, changes_count
        ),
        format!(
            "\"{}\" resources tracker detected {} changes. Visit {} to learn more.",
            tracker_name, changes_count, back_link
        ),
        api.templates.render(
            "resources_tracker_changes_email",
            &json!({
                "tracker_name": tracker_name,
                "changes_count": changes_count,
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
